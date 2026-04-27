Title: Add checkpoint support test
Files: src/evolve.rs, src/tests/evolve_checkpoint.rs
Issue: none

- Write a new integration test in `src/tests/evolve_checkpoint.rs` that verifies the checkpoint behavior:
  1. Create a temporary directory, run `run_evolve_with` to generate a checkpoint file, ensure the checkpoint file exists after the first run.
  2. Run `run_evolve_with` again; the assessment and planning phases should be skipped (can verify by checking that the checkpoint file remains and no new task files are recreated).
  3. After successful execution, ensure the checkpoint file is removed.
- The test should use `tempfile::TempDir` for isolation and clean up.
- Ensure the test imports necessary modules and adds `#[test]` attribute.
- Keep modifications limited to adding the test file and minimal changes to `src/evolve.rs` if needed for exposing checkpoint path (e.g., a helper function).