Title: Implement Assessment Phase Skeleton for Evolve Pipeline
Files: src/evolve.rs
Issue: none

Add a basic implementation of Phase A1 (Assessment) within `src/evolve.rs`. The function should:
- Define a new `fn assessment_phase() -> Result<String, Box<dyn std::error::Error>>` that collects simple self‑analysis data (e.g., reading the `Cargo.toml` version, counting source files, and checking if the latest CI passed via a placeholder). 
- Use a timeout of half the total evolve timeout (assume a constant `TIMEOUT_SECS: u64 = 1200`). Implement the timeout with `tokio::time::timeout` and return an error if it expires.
- Return a formatted string summary.
- Call this function from `run_evolve` and print the assessment result before exiting.
- Add necessary `use` statements (`tokio::time`, `std::fs`).
- Ensure the change compiles and the existing placeholder test `test_run_evolve_placeholder` still passes (it only checks for `Ok`).

Update `run_evolve` to propagate any errors from `assessment_phase`.

Write a unit test in `src/evolve.rs` (or a new test module) that verifies `assessment_phase` returns a non‑empty string when called.

After implementing, run `cargo test` to ensure all tests pass.
