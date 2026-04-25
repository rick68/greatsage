Title: Add tests for protected path enforcement in evolve pipeline
Files: src/evolve.rs, src/tests/protected_path.rs
Issue: none

Create a new test module `src/tests/protected_path.rs` that verifies:
1. `planning_phase` returns an error when the `session_plan` directory path is a protected location (e.g., create a temporary directory and symlink `session_plan` to a path inside `scripts`).
2. `execute_tasks` returns an error when a task file resides inside a protected path (e.g., create a task file under `skills/tmp.md`).
Use `tempfile::TempDir` and `std::os::unix::fs::symlink` (or Windows equivalent) to simulate the protected location.
Ensure the test cleans up temporary files.
