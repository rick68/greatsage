#!/usr/bin/env python3
"""
extract_trajectory.py — Build the YOUR TRAJECTORY block injected into Phase A1
(assess) and Phase A2 (plan) prompts. Aggregates audit-log session evidence,
git log, and gh run history into a structured markdown summary so greatsage sees
ground truth about its own recent trajectory before deciding what to work on.

Inputs (env vars):
  GREATSAGE_AUDIT_DIR       Path to audit-log worktree's `sessions/` directory.
  GREATSAGE_ITERATION       Current iteration number (used only for window calc + display).
  GREATSAGE_TRAJECTORY_OUT  Output file path. Default: .greatsage/session_staging/trajectory.md.

Output:
  Writes a single markdown blob to GREATSAGE_TRAJECTORY_OUT. ~1-2KB target, hard-capped
  at 100 lines / 2KB. Always exits 0; failure modes degrade per-section and write
  "(no trajectory data yet)" if no signal could be gathered.
"""
import json
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path

# ── Configuration constants ──────────────────────────────────────────────
WINDOW_SESSIONS = 10           # last N sessions in the outcomes section
WINDOW_ITERATIONS = 50         # iteration window for tracking activity
MAX_FAILED_RUNS = 5            # cap on `gh run view --log-failed` calls
GH_RUN_VIEW_TIMEOUT = 10       # seconds per gh run view
GH_RUN_LIST_TIMEOUT = 10       # seconds for gh run list
STUCK_ON_THRESHOLD = 3         # ≥N attempts AND 0 successes → flag
TOTAL_LINE_CAP = 100
TOTAL_BYTE_CAP = 2048

# ── Helpers ──────────────────────────────────────────────────────────────


def warn(msg: str) -> None:
    print(f"extract_trajectory: WARN: {msg}", file=sys.stderr)


def run_cmd(cmd: list[str], timeout: int = 10) -> tuple[int, str, str]:
    """Run a command, capture output. Returns (rc, stdout, stderr). Never raises.
    Uses start_new_session=True so a TimeoutExpired SIGKILLs the entire process
    group (including grandchildren like git/curl spawned by gh), not just the
    immediate child — prevents zombie buildup over many sessions."""
    try:
        r = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            start_new_session=True,
        )
        return r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired as e:
        warn(f"timed out after {timeout}s: {' '.join(cmd[:3])}...")
        # Best-effort kill of the whole process group; subprocess.run already
        # killed the immediate child but grandchildren may persist.
        try:
            if e.pid is not None:
                os.killpg(os.getpgid(e.pid), 9)  # SIGKILL
        except (ProcessLookupError, PermissionError, OSError):
            pass
        return 124, "", "timeout"
    except (FileNotFoundError, OSError) as e:
        warn(f"command failed: {' '.join(cmd[:3])}... — {e}")
        return 1, "", str(e)


def strip_ansi(s: str) -> str:
    return re.sub(r"\x1b\[[0-9;]*[a-zA-Z]", "", s)


def truncate_lines(s: str, n: int) -> str:
    lines = s.splitlines()
    if len(lines) <= n:
        return s
    return "\n".join(lines[:n] + [f"... ({len(lines) - n} more lines truncated)"])


# ── Section 1: Recent session outcomes ───────────────────────────────────


def load_outcomes(audit_dir: Path) -> list[dict]:
    """Read last N outcome.json files, sorted newest-first by mtime.
    Returns dicts unchanged from outcome.json — sort metadata is kept on a
    side tuple, never mutated into the parsed object (defends against keys
    like `_mtime` colliding with future schema additions)."""
    if not audit_dir.exists() or not audit_dir.is_dir():
        return []
    triples: list[tuple[float, str, dict]] = []
    for child in audit_dir.iterdir():
        if not child.is_dir():
            continue
        outcome = child / "outcome.json"
        if not outcome.is_file():
            continue
        try:
            data = json.loads(outcome.read_text(errors="replace"))
        except (OSError, json.JSONDecodeError, UnicodeDecodeError) as e:
            warn(f"skipped malformed {outcome}: {e}")
            continue
        try:
            mtime = outcome.stat().st_mtime
        except OSError as e:
            warn(f"could not stat {outcome}: {e}")
            mtime = 0.0
        triples.append((mtime, child.name, data))
    triples.sort(key=lambda t: t[0], reverse=True)
    # Return only the data dicts, but keep the original keys intact.
    return [t[2] for t in triples[:WINDOW_SESSIONS]]


def render_outcomes(outcomes: list[dict]) -> str:
    if not outcomes:
        return ""
    lines = ["## Recent session outcomes (last {})".format(len(outcomes))]
    for o in outcomes:
        iteration = o.get("iteration", "?")
        ts = (o.get("ts") or "").replace("T", " ").rstrip("Z")
        attempted = o.get("tasks_attempted", 0)
        succeeded = o.get("tasks_succeeded", 0)
        build_ok = o.get("build_ok", False)
        test_ok = o.get("test_ok", False)
        reverted = o.get("reverted", False)

        if reverted:
            icon = "❌"
            note = "REVERTED entire session"
        elif attempted == 0:
            icon = "•"
            note = "no tasks attempted"
        elif succeeded == attempted and build_ok and test_ok:
            icon = "✅"
            note = "build OK, tests OK"
        else:
            icon = "⚠️"
            issues = []
            if succeeded < attempted:
                issues.append(f"{attempted - succeeded} task(s) reverted")
            if not build_ok:
                issues.append("build broken")
            if not test_ok:
                issues.append("tests broken")
            note = ", ".join(issues) or "partial"

        lines.append(f"iteration-{iteration} ({ts}): tasks {succeeded}/{attempted} {icon} — {note}")
    return "\n".join(lines)


# ── Section 2: Per-task success rate from git log ────────────────────────


# Match commit messages like:
#   "Iteration 49 2026-04-26T21:35Z: Wire remaining useful bare subcommands (Task 3)"
#   "Iteration 57 2026-04-27T03:11Z: /watch multi-command support — run lint AND test in sequence (Task 2)"
TASK_COMMIT_RE = re.compile(
    r"^Iteration\s+(\d+)\s+\S+:\s+(.+?)\s+\(Task\s+\d+\)\s*$"
)
REVERT_COMMIT_RE = re.compile(
    r"^Iteration\s+\d+\s+\S+:\s+revert session changes", re.IGNORECASE
)


def collect_task_commits(current_iteration: int) -> tuple[list[tuple[int, str]], int]:
    """Return ([(iteration, title), ...], revert_commits_in_window).
    Fetches recent commits and filters by iteration range."""
    # Fetch last 200 commits to ensure we cover the iteration window
    rc, stdout, _ = run_cmd(
        ["git", "log", "-n", "200", "--format=%s"],
        timeout=15,
    )
    if rc != 0:
        return [], 0
    
    tasks = []
    reverts = 0
    min_iteration = current_iteration - WINDOW_ITERATIONS
    
    for line in stdout.splitlines():
        # Parse iteration and title from commit message
        m = TASK_COMMIT_RE.match(line)
        if m:
            it_num = int(m.group(1))
            if it_num >= min_iteration:
                tasks.append((it_num, m.group(2).strip()))
            continue
        
        # Check for session reverts within the iteration window
        if REVERT_COMMIT_RE.match(line):
            reverts += 1
            
    return tasks, reverts


def render_task_success(tasks: list[tuple[int, str]]) -> str:
    if not tasks:
        return ""
    # Group by title; count attempts.
    title_attempts: defaultdict[str, list[int]] = defaultdict(list)
    for iteration, title in tasks:
        title_attempts[title].append(iteration)

    lines = ["## Per-task activity (last {} iterations)".format(WINDOW_ITERATIONS)]
    stuck_titles = []
    for title, iterations in sorted(title_attempts.items(), key=lambda kv: -len(kv[1])):
        attempts = len(iterations)
        if attempts >= STUCK_ON_THRESHOLD:
            # Store iterations list to show range later
            stuck_titles.append((title, attempts, iterations))
        
        # Cap output at top 5 most-active titles
        if len(lines) > 6:
            continue
        last_iteration = max(iterations)
        truncated_title = title[:60] + ("…" if len(title) > 60 else "")
        lines.append(f"\"{truncated_title}\": {attempts} attempt(s), last iteration-{last_iteration}")

    if stuck_titles:
        lines.append("")
        lines.append(f"⚠️ Possibly stuck (≥{STUCK_ON_THRESHOLD} attempts in window):")
        for title, attempts, iterations in stuck_titles[:3]:
            t = title[:60] + ("…" if len(title) > 60 else "")
            # Display iteration range (min to max)
            lines.append(f"  - \"{t}\": {attempts}× (iterations {min(iterations)}-{max(iterations)})")
    return "\n".join(lines)


# ── Section 3: Reverts in window (already counted above) ─────────────────


def render_reverts(reverts: int, total_sessions: int) -> str:
    if total_sessions == 0:
        return ""
    if reverts == 0:
        return f"## Reverts in window\n0 of last ~{total_sessions} sessions had reverts."
    return f"## Reverts in window\n{reverts} revert commit(s) in last {WINDOW_ITERATIONS} iterations."


# ── Section 4: Recurring CI errors via gh run view --log-failed ──────────


ERROR_LINE_RE = re.compile(r"(error|panicked|FAILED|fatal)", re.IGNORECASE)


def fingerprint_error_line(line: str) -> str:
    """Normalize an error line to a clusterable fingerprint."""
    s = strip_ansi(line).strip()
    # Strip leading log timestamps and noisy prefixes
    s = re.sub(r"^\d{4}-\d{2}-\d{2}T?[\d:.,Z+ ]*\s*", "", s)
    s = re.sub(r"^[A-Za-z_-]+\s*[\|│]\s*", "", s)
    # Normalize file:line:column to file:N:N
    s = re.sub(r":\d+:\d+", ":N:N", s)
    s = re.sub(r":\d+\b", ":N", s)
    # Lowercase, collapse whitespace, truncate to 80 chars
    return re.sub(r"\s+", " ", s.lower())[:80]


def collect_failed_ci_fingerprints(repo: str) -> list[tuple[str, list[str]]]:
    """Return [(fingerprint, [run_ids_seen_at])]. Capped at MAX_FAILED_RUNS fetches.
    Silent return-empty paths now warn() so a misconfigured token / rate-limit
    doesn't masquerade as 'no failed runs' (would defeat the recurring-error
    detection this section exists for)."""
    if not repo:
        warn("GREATSAGE_REPO empty — skipping recurring-CI-error section")
        return []
    rc, stdout, stderr = run_cmd(
        [
            "gh", "run", "list", "--repo", repo,
            "--status", "failure", "--limit", str(MAX_FAILED_RUNS),
            "--json", "databaseId,createdAt,name,workflowName",
        ],
        timeout=GH_RUN_LIST_TIMEOUT,
    )
    if rc != 0:
        warn(f"gh run list rc={rc}: {(stderr or '').strip()[:200]}")
        return []
    try:
        runs = json.loads(stdout)
    except json.JSONDecodeError as e:
        warn(f"gh run list returned non-JSON: {e}")
        return []
    if not runs:
        return []

    fingerprints: defaultdict[str, list[str]] = defaultdict(list)
    fetch_errors = 0
    for run in runs:
        run_id = str(run.get("databaseId") or "")
        if not run_id:
            continue
        rc2, log_stdout, stderr2 = run_cmd(
            ["gh", "run", "view", run_id, "--repo", repo, "--log-failed"],
            timeout=GH_RUN_VIEW_TIMEOUT,
        )
        if rc2 != 0:
            fetch_errors += 1
            warn(f"gh run view {run_id} rc={rc2}: {(stderr2 or '').strip()[:120]}")
            continue
        tail = log_stdout.splitlines()[-50:]
        seen_in_run = set()
        for ln in tail:
            if ERROR_LINE_RE.search(ln):
                fp = fingerprint_error_line(ln)
                if fp and fp not in seen_in_run:
                    fingerprints[fp].append(run_id)
                    seen_in_run.add(fp)
    if fetch_errors and not fingerprints:
        warn(f"all {fetch_errors} gh run view fetch(es) failed — section will be empty")
    return sorted(fingerprints.items(), key=lambda kv: -len(kv[1]))


def render_ci_errors(clusters: list[tuple[str, list[str]]]) -> str:
    if not clusters:
        return ""
    lines = ["## Recurring CI errors (failed runs in window)"]
    for fp, run_ids in clusters[:5]:
        n = len(run_ids)
        marker = f"{n}×" if n > 1 else "1×"
        # Truncate fingerprint to keep line tidy
        fp_short = fp[:90]
        lines.append(f"[{marker}] {fp_short}")
    return "\n".join(lines)


# ── Section 5: Provider/API health from audit.jsonl files ────────────────


PROVIDER_ERROR_RE = re.compile(r'"type"\s*:\s*"error"|provider_error|rate_limit', re.IGNORECASE)


AUDIT_FILE_SIZE_CAP = 10 * 1024 * 1024  # 10MB per file — guard against runaway audit.jsonl


def collect_provider_errors(audit_dir: Path) -> tuple[int, int]:
    """Return (sessions_examined, total_provider_error_hits).
    Streams audit.jsonl line-by-line so a multi-MB file doesn't slurp into
    memory. Per-file size cap (10MB) protects against pathological cases."""
    if not audit_dir.exists():
        return 0, 0
    sessions = 0
    hits = 0
    for child in sorted(audit_dir.iterdir(), reverse=True):
        if not child.is_dir():
            continue
        audit = child / "audit.jsonl"
        if not audit.is_file():
            continue
        sessions += 1
        try:
            size = audit.stat().st_size
            if size > AUDIT_FILE_SIZE_CAP:
                warn(f"{audit} is {size} bytes (>{AUDIT_FILE_SIZE_CAP}); scanning first {AUDIT_FILE_SIZE_CAP}B only")
            with audit.open(encoding="utf-8", errors="replace") as f:
                bytes_read = 0
                for line in f:
                    bytes_read += len(line)
                    if bytes_read > AUDIT_FILE_SIZE_CAP:
                        break
                    if PROVIDER_ERROR_RE.search(line):
                        hits += 1
        except OSError as e:
            warn(f"skipped {audit}: {e}")
        if sessions >= WINDOW_SESSIONS:
            break
    return sessions, hits


def render_provider_health(sessions: int, hits: int) -> str:
    if sessions == 0:
        return ""
    if hits == 0:
        return f"## Provider/API health\n{sessions} sessions, no provider errors detected."
    return f"## Provider/API health\n{sessions} sessions, {hits} provider error hit(s) in audit.jsonl."


# ── Final assembly ───────────────────────────────────────────────────────


def main() -> int:
    audit_dir_str = os.environ.get("GREATSAGE_AUDIT_DIR", "")
    repo = os.environ.get("GREATSAGE_REPO", "")
    iteration_str = os.environ.get("GREATSAGE_ITERATION", "0")
    try:
        current_iteration = int(iteration_str)
    except ValueError:
        current_iteration = 0
        
    out_path_str = os.environ.get(
        "GREATSAGE_TRAJECTORY_OUT", ".greatsage/session_staging/trajectory.md"
    )
    out_path = Path(out_path_str)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    # Drop any stale output from a prior session
    try:
        out_path.unlink()
    except FileNotFoundError:
        pass
    except OSError as e:
        warn(f"could not unlink stale {out_path}: {e}")

    audit_dir = Path(audit_dir_str) if audit_dir_str else Path("/dev/null")

    header = (
        f"# YOUR TRAJECTORY\n\n"
        f"Last computed: {datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%MZ')}. "
        f"Iteration {current_iteration}. Window: last {WINDOW_SESSIONS} sessions / {WINDOW_ITERATIONS} iterations.\n"
    )

    # Gather all sections (each falls back to "" silently on no-data)
    outcomes = load_outcomes(audit_dir)
    tasks, reverts = collect_task_commits(current_iteration)
    sessions_audited, provider_hits = collect_provider_errors(audit_dir)
    ci_clusters = collect_failed_ci_fingerprints(repo)

    sections: list[str] = []
    s = render_outcomes(outcomes)
    if s:
        sections.append(s)
    s = render_task_success(tasks)
    if s:
        sections.append(s)
    s = render_reverts(reverts, len(outcomes))
    if s:
        sections.append(s)
    s = render_ci_errors(ci_clusters)
    if s:
        sections.append(s)
    s = render_provider_health(sessions_audited, provider_hits)
    if s:
        sections.append(s)

    if not sections:
        body = "(no trajectory data yet — audit-log is empty and no recent task commits found)"
    else:
        body = "\n\n".join(sections)

    output = header + "\n" + body + "\n"
    # Hard-cap: lines and bytes. Bytes-cap reserves room for the truncation
    # marker so the FINAL output stays under TOTAL_BYTE_CAP (the marker
    # itself was previously appended after the cap, allowing the file to
    # exceed it by ~37 bytes).
    output = truncate_lines(output, TOTAL_LINE_CAP)
    truncation_marker = "\n... (truncated to fit token budget)\n"
    marker_bytes = len(truncation_marker.encode("utf-8"))
    if len(output.encode("utf-8")) > TOTAL_BYTE_CAP:
        budget = TOTAL_BYTE_CAP - marker_bytes
        b = output.encode("utf-8")[:budget]
        # Back off to last newline within b for clean cut
        idx = b.rfind(b"\n")
        if idx > 0:
            b = b[:idx]
        output = b.decode("utf-8", errors="ignore") + truncation_marker

    try:
        out_path.write_text(output)
    except OSError as e:
        warn(f"could not write {out_path}: {e}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
