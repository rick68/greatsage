Title: Add tests for PermissionConfig path validation
Files: src/agents/mod.rs
Issue: none

## Description
Implement unit tests for the `PermissionConfig` struct to verify its path validation logic.

### Steps
1. In `src/agents/mod.rs`, locate the existing `#[cfg(test)] mod tests` module.
2. Add two new test functions:
   - `fn permission_allows_path_within_cwd()` – create a temporary directory, set `allowed_dir` to it, create a sub‑path file inside, and assert that `validate_path` returns `Ok(())`.
   - `fn permission_denies_path_outside_cwd()` – create a second temporary directory outside the allowed one, construct a path inside it, and assert that `validate_path` returns `Err` with the appropriate message.
3. Use `tempfile` crate (already a dev‑dependency via Cargo.lock) or `std::env::temp_dir` to create isolated directories.
4. Ensure the tests compile and run with `cargo test`.
5. No runtime behavior changes; only test coverage improves.

### Documentation
Update `README.md` under a new "Security" section briefly describing that the agent now has verified permission checks for file system access.

### Verification
Run `cargo test`. All existing tests plus the new ones should pass.
