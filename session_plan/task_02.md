Title: Add checkpoint support to evolve pipeline
Files: src/evolve.rs
Issue: none

- Add a simple checkpoint file `.greatsage/evolve_checkpoint` at the start of `run_evolve_with`.
- If the checkpoint file exists, skip the assessment and planning phases and directly proceed to `execute_tasks`.
- After successful execution, remove the checkpoint file.
- Ensure the implementation touches only `src/evolve.rs` and does not affect existing behavior.
- Add inline comments explaining the purpose of the checkpoint.
