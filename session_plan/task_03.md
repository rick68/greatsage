Title: Implement basic build/test fix loop in evolve pipeline
Files: src/evolve.rs
Issue: none

**Description**
Add a simple fix‑loop that attempts to build and test the project after each task implementation.

- Create a function `fn run_fix_loop(task_id: usize) -> Result<()>` that performs up to 10 attempts:
  1. Runs `cargo build`.
  2. If build succeeds, runs `cargo test`.
  3. If both succeed, returns `Ok(())`.
  4. If either fails, captures the error output, optionally logs it, and retries after a short delay (e.g., 1 second).
- The loop should stop early on success. On exhausting attempts, return an error indicating the task failed to produce a build‑stable state.
- Integrate this function into the evolve subcommand so that after each task’s code modifications (once they are applied), the fix loop runs before moving to the next task.

**Tests**
Create unit tests (or integration tests) in `tests/evolve_fix_loop.rs` that simulate failure and success scenarios by mocking the command execution (you may use a simple wrapper around `std::process::Command` that can be overridden in tests). Verify that:
- The loop retries up to the limit on failure.
- It exits early on success.
- Proper error is returned after exhausting attempts.

**Documentation**
Update the README section “Evolution Pipeline – Fix Loop” describing the 10‑attempt build/test retry behavior.
