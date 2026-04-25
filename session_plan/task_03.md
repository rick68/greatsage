Title: Add unit tests for protected file verification in evolve pipeline
Files: src/tests/protected_paths.rs
Issue: none

Create a new test module `src/tests/protected_paths.rs` that imports `crate::evolve::verify_protected_paths`. Write tests covering:
- An empty vector returns Ok.
- A vector containing a protected path like `.github/workflows/.github` returns Err with a message mentioning the protected path.
- A vector with a non‑protected path (e.g., `src/main.rs`) returns Ok.
Ensure the tests run with `cargo test`. No changes to production code beyond the `verify_protected_paths` function are needed.
