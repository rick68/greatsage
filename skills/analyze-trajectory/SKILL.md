---
name: analyze-trajectory
description: Diagnose a recurring failure (STUCK task, clustered CI error, frequent reverts) by dispatching sub-agents to digest CI logs without bloating main context. Returns one root-cause diagnosis.
tools: [bash, read_file, sub_agent]
core: true
origin: creator
---

# Analyze Trajectory

You are doing a **deep dive** into a recurring failure pattern. The harness's pre-computed `YOUR TRAJECTORY` block surfaces *that* something is recurring; this skill helps you understand *why* and produce a focused diagnosis.

This skill exists because raw GitHub Actions logs are too large and noisy to digest in your main context window. The pattern (Recursive Language Model — see Reithan's reference in issue #226) is: keep your root context small, dispatch a sub-agent to read the raw logs, and have the sub-agent return a 1-3 sentence summary. Recurse if the summary surfaces a deeper question.

## When to use

Trigger this skill when ANY of these hold:

- `YOUR TRAJECTORY` flagged a `STUCK` task (≥3 attempts in window, 0 successes)
- A CI error fingerprint appeared `≥2×` in the recurring-errors section
- Multiple revert commits appeared across recent sessions (the trajectory's "Reverts in window" line shows the count)
- A specific issue (e.g. `#205`) has been mentioned in multiple session journals without resolution

## When NOT to use

- The trajectory looks healthy. Don't spelunk for problems that aren't there — that's just burning sub-agent budget.
- The failure is well-understood already (you already know the cause from journal/learnings). Skip straight to the fix.
- You're inside Phase B (implementation) and the failure is the task you're currently doing — fix it directly, don't recurse.

## Procedure

### 1. Frame the question (single sentence)

Examples of well-framed questions:
- *"Why does the evaluator phase fail with 'AnthropicError: rate_limit_exceeded' on sessions day-53, day-55, and day-56?"*
- *"Why was the task 'Add /fallback flag' reverted on 6 separate sessions? What's the recurring blocker?"*
- *"What does run 4321 look like at the moment of failure?"*

A good question names a specific event (run id, session day, error fingerprint) and what you want to know about it. Don't ask vague questions like *"what's wrong with my trajectory?"*

### 2. Identify the artifact

For each question, pick exactly one artifact to fetch:

- **CI failure** → run id from the trajectory's CI errors section. `gh run view <id> --log-failed` (drop `--repo`; gh auto-detects from the local clone's origin remote, which is the right one)
- **Reverted task** → commit SHA of the revert. `git show <sha>` and the next-newer commit's full diff
- **Session-level wreckage** → audit.jsonl from that session. **Note**: `$GREATSAGE_AUDIT_DIR` is set by the harness ONLY inside `scripts/skill_evolve.sh` (a different invocation than evolve.sh). When loaded inside a normal evolve session, you must fetch the audit-log branch yourself first:
  ```bash
  git fetch --depth 50 origin audit-log:audit-log
  AUDIT_WT=$(mktemp -d)
  git worktree add "$AUDIT_WT" audit-log
  ls "$AUDIT_WT/sessions/" | tail -10
  # ... read what you need ...
  git worktree remove --force "$AUDIT_WT"
  ```

### 3. Decide: direct read or sub-agent?

Estimate the artifact size first:
```bash
gh run view <id> --log-failed 2>/dev/null | wc -c
```

- **< 5KB**: read it directly with `read_file` or `bash`. Skip sub-agent — the cost isn't worth it.
- **≥ 5KB**: dispatch a sub-agent. Don't load raw logs into your main context.

### 4. Dispatch a sub-agent (if needed)

Use the `sub_agent` tool with this template. The sub-agent must return JSON conforming to this exact schema — note the `null` examples (JSON null, not the string `"null"`):

```
Question: <your single-sentence question from step 1>

Artifact (compressed log; do NOT include this in your reply, only summarize):
<paste the gh run view output here>

Reply with ONLY a JSON object (no markdown fences, no prose) matching this schema:

{
  "summary": "1-3 sentences explaining the root cause, with no surrounding quotes",
  "key_lines": ["file.rs:42:11 borrow of moved value", "AnthropicError: rate_limit_exceeded"],
  "deeper_question": null,
  "confidence": "medium"
}

Field rules:
- summary: free string, 1-3 sentences
- key_lines: array of 1-5 short strings (max 100 chars each) that prove the cause
- deeper_question: JSON null when no follow-up is needed; otherwise a single-sentence string
- confidence: exactly one of "high", "medium", or "low"
```

Sub-agents inherit RTK compression on bash output and directory restrictions, but they do NOT inherit skills. Keep the sub-agent prompt fully self-contained — don't reference other skills.

**Sub-agent failure fallback** — if the sub-agent (a) errors, (b) returns non-JSON, (c) returns truncated JSON, or (d) is unavailable as a tool:

1. Append the raw response to `memory/learnings.jsonl` as a learning entry with `pattern_key: trajectory.subagent_malformed_response` so we can debug later.
2. Downgrade to a direct read of the artifact: `read_file` or `bash`-tail the last 50-100 lines of the log into your main context.
3. Produce a low-confidence diagnosis from what you can see directly. Skip recursion (no point — sub-agent path is broken).
4. Mark the diagnosis with `confidence: low (sub-agent unavailable)` so downstream decisions know to be cautious.

### 5. Recurse if the sub-agent returns `deeper_question`

If `confidence` is `"low"` AND `deeper_question` is a non-null string (JSON null returns false on this check, but if you see the literal string `"null"` treat it as null too — that's a sub-agent bug worth logging), run another sub-agent dispatch with the narrower question. Reuse the same artifact; the sub-agent will focus differently.

**Hard cap: recursion depth = 3.** That's: initial dispatch → 1st recursion → 2nd recursion. After that, accept whatever you have. The cap is informed by the recursive-LM literature ([RLM blog, alexzhang13.github.io/blog/2025/rlm/](https://alexzhang13.github.io/blog/2025/rlm/)) and prevents runaway agent costs.

If you hit the cap without `confidence == "high"`, that's still a valid outcome — write the diagnosis with whatever clarity you have and flag it as "needs follow-up".

### 6. Aggregate to a single diagnosis

Produce a 3-5 sentence diagnosis paragraph that includes:
- **What recurs**: one-line summary of the pattern
- **Root cause** (or best-guess): from the sub-agent's summary
- **Evidence**: ≤3 specific lines or run IDs
- **Suggested next attempt**: one concrete action (a different approach, a new task, or "log to learnings.jsonl and skip for now")

Write the diagnosis somewhere durable:
- If you're in a normal evolve session and this informed your task choice → cite it in the assessment doc
- If you're investigating a specific issue → comment on the issue with the diagnosis
- Always also append a `learnings.jsonl` entry. The `pattern_key` field (optional in the standard schema, see `skills/communicate/SKILL.md`) takes a kebab-case `<verb>.<object>` value — for trajectory-derived diagnoses, use `pattern_key: trajectory.<short-slug>` (e.g., `trajectory.fallback_provider_stuck`, `trajectory.evaluator_rate_limit`). This lets skill-evolve cluster recurring trajectory findings.

## Pitfalls

- **Don't ask the sub-agent to make decisions.** It summarizes evidence; you decide what to do. Sub-agents in chained recursion can drift if asked to plan.
- **Don't recurse on `confidence: high`.** The whole point is to stop early when you have a clear answer.
- **Don't dump multiple artifacts to one sub-agent.** One artifact per dispatch keeps the sub-agent focused and the JSON output reliable.
- **Don't forget the recursion cap.** 3 is the hard limit. If you find yourself wanting depth 4, your initial question was probably too vague — go back to step 1.
- **Skills do not chain.** Sub-agents don't load this skill or any other; you must paste the question + artifact into the sub-agent's prompt directly.
- **Don't run this skill inside Phase B (implementation).** That's task-execution time, not introspection time. Save the diagnosis for the next session's Phase A1 (assess).

## Verification

A diagnosis is "good enough" when ALL of:
- It names a concrete file/line/condition (not "something with the API")
- It cites at least one specific run id or commit SHA
- The suggested next attempt is *different* from what's already been tried (otherwise you'll just hit the same wall)
- The total work used ≤3 sub-agent dispatches

If the diagnosis fails any of these, recurse one more time (within the cap) or accept the partial result and document the open question in `learnings.jsonl`.

## What this skill deliberately does NOT do

- **Does not modify code.** Diagnosis is the output. The actual fix is a normal task on a future evolve session — it's better to step away with the diagnosis written down and let the next session's planning agent decide whether to act on it.
- **Does not auto-create issues.** If the diagnosis is worth filing, do it via `communicate` skill in the same session — but it's a separate decision, not part of this skill's procedure.
- **Does not write to `audit-log` branch.** The branch is read-only from this skill's perspective.
