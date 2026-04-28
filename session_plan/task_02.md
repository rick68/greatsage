Title: Add tests for protected‑file enforcement in evolve pipeline
Files: src/evolve.rs, tests/protected_path_test.rs
Issue: none

Create unit tests that verify `is_protected_path` correctly identifies prohibited paths and allows legitimate ones. The test file should:
1. Import the `is_protected_path` function.
2. Assert that paths like `.github/workflows/build.yml`, `IDENTITY.md`, `scripts/evolve.sh`, and any file under `skills/` return `true`.
3. Assert that normal source files such as `src/main.rs` or `README.md` return `false`.
4. Ensure the function works on both relative and absolute path inputs.

Add the test module under `tests/` (or in `src/evolve.rs` with `#[cfg(test)]`). Run `cargo test` to confirm all pass.

Update the CI configuration if needed to include the new test file.
