Title: Scaffold full evolve pipeline phases
Files: src/evolve.rs
Issue: none

## Description
The evolve pipeline currently contains placeholder tasks and minimal phases (assessment, planning, execute). To move towards parity with Claude Code, we need to flesh out the full pipeline:

1. **Phase A2 (Planning)** – generate up to three task markdown files based on self‑assessment, open issues, and sponsor priorities.
2. **Phase B (Implementation)** – for each task, run a build/test fix loop (up to 10 attempts) and an evaluator fix loop (up to 9 attempts), with checkpoint‑restart on interruption and protected‑path verification.
3. **Phase C (Response)** – comment on and close related GitHub issues via the CLI.
4. **Wrap‑up** – write a journal entry, update iteration counter, and push changes.

### Goals for this task
- Extend `src/evolve.rs` with function stubs for each sub‑phase (planning_phase already exists, but expand it to generate tasks based on real data).
- Add a new function `run_task(task_path: &Path) -> Result<(), Error>` that will later contain the fix loops.
- Ensure the new functions are compiled (no‑op bodies returning `Ok(())`).
- Add unit tests in `src/tests/evolve_scaffold.rs` that verify the new functions exist and can be called without error.

### Acceptance Criteria
- The code builds with `cargo build`.
- New tests pass.
- No runtime behavior change yet; this is scaffolding for future implementation.
