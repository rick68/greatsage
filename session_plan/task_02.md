Title: Implement build/test fix loop in evolve pipeline
Files: src/evolve.rs, src/tests/build_test_fix_loop.rs
Issue: none

Create a helper function `fn build_test_fix_loop() -> Result<(), Box<dyn std::error::Error>>` that runs `cargo build` and `cargo test` sequentially using `std::process::Command`. If either command fails, retry up to 10 attempts, sleeping 1 second between attempts. Log each attempt to `.greatsage/evolve.log`. Integrate this function into `run_evolve_with` after the planning phase, before executing tasks.

Add a test `src/tests/build_test_fix_loop.rs` that invokes `build_test_fix_loop` on the current repository (which should succeed) and asserts it returns `Ok(())`.
