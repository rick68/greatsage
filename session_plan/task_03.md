Title: Add tests for evolve.rs protected path logic
Files: src/tests/evolve_protection.rs, src/evolve.rs
Issue: none

Create a new test module in `src/tests/evolve_protection.rs` that verifies `is_protected_path` correctly identifies protected directories (e.g., `.github/workflows`, `IDENTITY.md`, `scripts`, `skills`) and does NOT incorrectly flag similar names like `scripts_backup` or files inside other directories. Ensure the test builds and passes with `cargo test`.
