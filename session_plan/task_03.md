Title: Add tests for protected‑path guard edge cases
Files: src/evolve.rs, src/tests.rs
Issue: none

Create unit tests that verify `is_protected_path` correctly identifies protected directories even with various path normalizations (e.g., trailing slashes, relative components like `./.github/workflows/..`, case variations). Ensure the guard rejects modifications to these paths while allowing legitimate ones. Add the tests to the existing test suite and update any needed imports.
