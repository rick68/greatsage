//! Shell helpers for `/run`, `!`, and `/cd` (yoyo parity; no LLM).

use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Result of running a shell command via `/run` or `!`.
#[derive(Debug, Clone)]
pub(crate) struct RunResult {
    /// Original command string (kept for future `/fix`).
    #[allow(dead_code)]
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
    pub success: bool,
}

/// Process-scoped snapshot stored for a future `/fix` change.
pub(crate) type LastFailedRun = RunResult;

/// Parse a bang line after optional leading whitespace.
///
/// - `None` — not a bang line
/// - `Some("")` — bare `!` / `!   ` (usage)
/// - `Some(body)` — command body (`!!` → `"!"`)
pub(crate) fn parse_bang_command(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix('!')?;
    Some(rest.trim())
}

/// Run `sh -c <cmd>`, collecting stdout/stderr (buffered; no LLM).
pub(crate) fn run_shell_command(cmd: &str) -> RunResult {
    let start = Instant::now();
    let output = Command::new("sh")
        .args(["-c", cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(out) => {
            let exit_code = out.status.code().unwrap_or(-1);
            let success = out.status.success();
            RunResult {
                command: cmd.to_string(),
                exit_code,
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                elapsed: start.elapsed(),
                success,
            }
        }
        Err(e) => RunResult {
            command: cmd.to_string(),
            exit_code: -1,
            stdout: String::new(),
            stderr: format!("error running command: {e}"),
            elapsed: start.elapsed(),
            success: false,
        },
    }
}

pub(crate) fn format_run_duration(elapsed: Duration) -> String {
    let ms = elapsed.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", elapsed.as_secs_f64())
    }
}

/// Usage lines for bare `/run` or bare `!` (yoyo-style consecutive output).
pub(crate) fn run_usage_lines() -> Vec<String> {
    vec![
        "usage: /run <command>  or  !<command>".to_string(),
        "Runs a shell command directly (no AI, no tokens).".to_string(),
    ]
}

/// Max lines for yoyo-style failure re-preview under `✗ exit` (before 💡 tip).
const RUN_FAIL_PREVIEW_MAX: usize = 3;

/// Leading spaces for failure re-preview lines (yoyo `print_run_result`: 4 spaces).
const RUN_FAIL_PREVIEW_INDENT: &str = "    ";

fn is_error_like_stdout_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("error") || lower.contains("failed") || lower.contains("panic")
}

/// yoyo: up to 3 stderr lines, else up to 3 error-like stdout lines; each with 4-space indent.
fn failure_preview_lines(result: &RunResult) -> Vec<String> {
    let mut preview: Vec<String> = result
        .stderr
        .lines()
        .take(RUN_FAIL_PREVIEW_MAX)
        .map(|l| format!("{RUN_FAIL_PREVIEW_INDENT}{l}"))
        .collect();
    if preview.is_empty() {
        preview = result
            .stdout
            .lines()
            .filter(|l| is_error_like_stdout_line(l))
            .take(RUN_FAIL_PREVIEW_MAX)
            .map(|l| format!("{RUN_FAIL_PREVIEW_INDENT}{l}"))
            .collect();
    }
    preview
}

/// Lines to print for a completed run.
///
/// Order: stdout → stderr → `✓`/`✗ exit` → (fail: re-preview ≤3 lines) → (fail: 💡 tip).
/// No blank line before exit. Preview uses 4-space indent (yoyo).
pub(crate) fn format_run_output_lines(result: &RunResult) -> (Vec<String>, Vec<String>) {
    let mut output: Vec<String> = result.stdout.lines().map(str::to_string).collect();
    () = output.extend(result.stderr.lines().map(str::to_string));

    let elapsed = format_run_duration(result.elapsed);
    if result.success {
        () = output.push(format!("✓ exit {} ({elapsed})", result.exit_code));
    } else {
        () = output.push(format!("✗ exit {} ({elapsed})", result.exit_code));
        // yoyo: dim re-preview of stderr (or error-like stdout) under exit, then 💡 tip.
        () = output.extend(failure_preview_lines(result));
        () = output.push(
            "💡 Command failed. Ask me to analyze the error, or say /fix to auto-fix.".to_string(),
        );
    }
    // No `detail` — keeps exit after all command text in write_repl_handled_output.
    (output, Vec::new())
}

pub(crate) fn current_directory_display() -> String {
    env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string())
}

/// yoyo note after a successful `/cd <path>` (project context not reloaded).
pub(crate) const CD_CONTEXT_NOTE: &str = "(project context was loaded from the original \
directory and is not reloaded — use /context to review)";

/// Success lines for `/cd <path>`: path (white in terminal) + dim context note.
pub(crate) fn cd_success_lines(new_cwd: &std::path::Path) -> Vec<String> {
    vec![new_cwd.display().to_string(), CD_CONTEXT_NOTE.to_string()]
}

/// Expand `~` / `~/…` and resolve relative paths against the process cwd.
pub(crate) fn expand_cd_path(raw: &str) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("empty path".to_string());
    }

    let expanded = if raw == "~" {
        home_dir().ok_or_else(|| "HOME is not set".to_string())?
    } else if let Some(rest) = raw.strip_prefix("~/") {
        let home = home_dir().ok_or_else(|| "HOME is not set".to_string())?;
        home.join(rest)
    } else {
        PathBuf::from(raw)
    };

    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        let cwd = env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?;
        Ok(cwd.join(expanded))
    }
}

pub(crate) fn change_directory(raw: &str) -> Result<PathBuf, String> {
    let path = expand_cd_path(raw)?;
    if !path.exists() {
        return Err(format!("no such file or directory: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("not a directory: {}", path.display()));
    }
    () =
        env::set_current_dir(&path).map_err(|e| format!("cannot cd to {}: {e}", path.display()))?;
    env::current_dir().map_err(|e| format!("cd succeeded but cannot read cwd: {e}"))
}

fn home_dir() -> Option<PathBuf> {
    dirs::home_dir().or_else(|| env::var_os("HOME").map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bang_basic() {
        assert_eq!(parse_bang_command("!ls -la"), Some("ls -la"));
        assert_eq!(parse_bang_command("  !git status"), Some("git status"));
        assert_eq!(parse_bang_command("!echo hi  "), Some("echo hi"));
    }

    #[test]
    fn parse_bang_empty_and_double() {
        assert_eq!(parse_bang_command("!"), Some(""));
        assert_eq!(parse_bang_command("!   "), Some(""));
        assert_eq!(parse_bang_command("!!"), Some("!"));
    }

    #[test]
    fn parse_bang_not_bang() {
        assert_eq!(parse_bang_command("hello"), None);
        assert_eq!(parse_bang_command("/help"), None);
        assert_eq!(parse_bang_command("wow!"), None);
    }

    #[test]
    fn run_shell_echo() {
        let r = run_shell_command("echo hello-shell");
        assert!(r.success);
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello-shell"));
    }

    #[test]
    fn run_shell_nonzero() {
        let r = run_shell_command("false");
        assert!(!r.success);
        assert_ne!(r.exit_code, 0);
    }

    #[test]
    fn format_run_puts_exit_after_stdout_and_stderr() {
        let r = RunResult {
            command: "x".into(),
            exit_code: 1,
            stdout: "out\n".into(),
            stderr: "err\n".into(),
            elapsed: Duration::from_millis(1),
            success: false,
        };
        let (lines, detail) = format_run_output_lines(&r);
        assert!(detail.is_empty(), "exit must not use detail channel");
        let exit_i = lines
            .iter()
            .position(|l| l.starts_with("✗ exit "))
            .expect("exit line");
        let out_i = lines.iter().position(|l| l == "out").unwrap();
        let err_i = lines.iter().position(|l| l == "err").unwrap();
        assert!(
            out_i < err_i && err_i < exit_i,
            "order: stdout, stderr, exit — {lines:?}"
        );
        assert_eq!(exit_i, err_i + 1, "no blank line before exit: {lines:?}");
    }

    #[test]
    fn format_run_failure_preview_before_tip() {
        let r = RunResult {
            command: "ps".into(),
            exit_code: 1,
            stdout: String::new(),
            stderr: "ps: illegal option -- -\nusage: ps [-AaCcEefhjlMmrSTvwXx]\n          [-g grp]\n       ps [-L]\n".into(),
            elapsed: Duration::from_millis(10),
            success: false,
        };
        let (lines, _) = format_run_output_lines(&r);
        let exit_i = lines.iter().position(|l| l.starts_with("✗ exit ")).unwrap();
        let tip_i = lines
            .iter()
            .position(|l| l.starts_with("💡 Command failed."))
            .expect("tip");
        let preview: Vec<&String> = lines[exit_i + 1..tip_i].iter().collect();
        assert_eq!(
            preview.len(),
            3,
            "yoyo takes up to 3 preview lines: {lines:?}"
        );
        assert!(
            preview.iter().all(|l| l.starts_with("    ")),
            "preview lines use 4-space indent: {preview:?}"
        );
        assert!(preview[0].contains("illegal option"));
        assert_eq!(tip_i, exit_i + 1 + preview.len());
    }

    #[test]
    fn expand_tilde_home() {
        if home_dir().is_none() {
            return;
        }
        let p = expand_cd_path("~").expect("expand ~");
        assert_eq!(p, home_dir().unwrap());
    }

    #[test]
    fn format_duration_units() {
        assert_eq!(format_run_duration(Duration::from_millis(42)), "42ms");
        assert_eq!(format_run_duration(Duration::from_millis(1500)), "1.5s");
    }
}
