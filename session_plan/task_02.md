Title: Add checkpoint‑restart support to evolve pipeline
Files: src/evolve.rs
Issue: none

**Description**
Implement a simple checkpoint‑restart mechanism used by the evolution pipeline.

- Add a function `fn save_checkpoint(task_id: usize) -> Result<()>` that records the current Git HEAD commit hash (using the existing `git::current_head()` helper) and the task identifier to a temporary file such as `.evolve_checkpoint`.
- Add a function `fn load_checkpoint() -> Option<(usize, String)>` that reads the checkpoint file, returning the saved task id and commit hash if the file exists.
- At the start of each task execution in the evolve subcommand, call `load_checkpoint()`. If a checkpoint is present and the current HEAD differs from the saved hash, abort with a warning that the repository changed since the last run.
- After a task completes successfully, remove the checkpoint file.
- If the process is interrupted (e.g., receives a termination signal), the checkpoint file remains, allowing the next run to resume from the pending task.

**Tests**
Create unit tests in `src/evolve.rs` that verify:
- `save_checkpoint` correctly writes the file with the expected format.
- `load_checkpoint` reads back the same values.
- The functions handle missing files gracefully (return `None`).
- The checkpoint is cleared after successful task completion.

**Documentation**
Update `README.md` (or a new “Evolution Pipeline – Checkpoint‑Restart” section) describing the new safety feature and how users can resume an interrupted evolution run.
