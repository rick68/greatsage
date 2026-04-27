Title: Add checkpoint‑restart support to the evolve pipeline
Files: src/evolve.rs
Issue: none

Implement a basic checkpoint‑restart mechanism for the evolve process. When `run_evolve_with` starts, create a checkpoint file (e.g., `.greatsage/evolve_checkpoint.json`) recording the current phase (assessment, planning, execution) and any generated task metadata. If the process is interrupted (simulated by a panic or early exit), subsequent runs should detect the checkpoint, restore the previous state, and resume from the point of interruption, with a maximum of two attempts per phase. Update `run_evolve_with` to check for an existing checkpoint at startup and to write/update the checkpoint after each successful phase. Ensure the checkpoint file is stored safely (respect protected‑path checks) and is cleaned up on successful completion. Add unit tests in `tests/checkpoint_restart.rs` to verify that a simulated interruption causes the pipeline to resume correctly on the next run.
