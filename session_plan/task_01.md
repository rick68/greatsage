Title: Implement assessment phase for evolve subcommand
Files: src/evolve.rs, src/git.rs, src/config.rs
Issue: none

Description:
- In `src/evolve.rs`, implement the `assessment_phase()` function.
- Use the existing `git` module to run `cargo build` and `cargo test`, capturing exit status, stdout, and stderr.
- Define a struct `AssessmentResult` with fields:
  - `build_success: bool`
  - `test_success: bool`
  - `test_passed: usize`
  - `test_failed: usize`
  - `timestamp: String` (ISO 8601)
- Serialize this struct to JSON and write it to the path defined by the config option `assessment_output_path` (default `target/assessment.json`).
- Return `Result<AssessmentResult, anyhow::Error>` and integrate the call into `run_evolve()` so the assessment runs at the start of the evolve pipeline.
- Add a unit test in `src/tests/evolve_assessment.rs` that runs the function in a temporary directory (using `tempfile` crate) and asserts that the JSON file is created and contains the expected fields.
- Limit modifications to the three listed files.
