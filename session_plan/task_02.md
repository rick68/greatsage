Title: Add unit tests for protected path checking
Files: src/evolve.rs, tests/protected_path_tests.rs
Issue: none

Create comprehensive unit tests for the `is_protected_path` function covering:
- Detection of protected directories like `.github/workflows`.
- Detection of protected files such as `IDENTITY.md`.
- Ensure non‑protected paths (e.g., `src/main.rs`, `scripts_backup/foo.rs`) return false.
- Test edge cases with symlinks or nested components.
Add the test file under `tests/` (creating the directory if needed) and ensure it runs with `cargo test`. Update `Cargo.toml` if needed to include the test module (not required for integration tests).