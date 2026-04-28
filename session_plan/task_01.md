Title: Implement checkpoint creation for each task in evolve pipeline
Files: src/evolve.rs
Issue: none

## Description
Add logic in `execute_tasks` to create a checkpoint before processing each task. The checkpoint should capture the current Git HEAD SHA and store it in `.greatsage/checkpoint_<task_number>.txt`. This provides a recovery point for the evolve pipeline.

Steps:
1. At the start of the loop over `entries` in `execute_tasks`, determine the task index (1‑based).
2. Run `git rev-parse HEAD` in the base directory and write the output to `.greatsage/checkpoint_<index>.txt`.
3. Ensure the `.greatsage` directory exists.
4. Log the checkpoint creation to the evolve.log.

Update any related documentation (e.g., README mentions checkpoint‑restart).
