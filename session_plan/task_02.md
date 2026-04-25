Title: Add build and test fix loop skeleton to evolve pipeline
Files: src/evolve.rs
Issue: none

Create a new helper `fn build_test_fix_loop() -> Result<(), Box<dyn std::error::Error>>` that runs `cargo build` and `cargo test` with up to 10 attempts, each time retrying after a short delay if they fail. Integrate this function into `run_evolve_with` after the planning phase so the evolve pipeline performs a basic fix‑loop before exiting.

Add a unit test to verify that the function returns Ok when the current project builds and tests successfully.
