Title: Add tests for protected‑file enforcement in evolve pipeline
Files: src/evolve.rs, tests/protected_path_test.rs
Issue: none

## Description
Ensure the `is_protected_path` function correctly blocks modifications to critical directories.

### Steps
1. In `src/evolve.rs` confirm the `is_protected_path` function is public (or add `pub(crate)` if needed) so tests can call it.
2. Create a new test file `tests/protected_path_test.rs` with unit tests:
   - Assert `is_protected_path(".github/workflows/ci.yml")` returns `true`.
   - Assert `is_protected_path("src/main.rs")` returns `false`.
   - Test a nested protected path like `scripts/evolve.sh`.
3. Run `cargo test` to verify all tests pass.

The change touches exactly two files and can be verified with the test suite.
