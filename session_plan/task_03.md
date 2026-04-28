Title: Update README with checkpoint‑restart documentation
Files: README.md
Issue: none

## Description
Add a new section to `README.md` describing the checkpoint‑restart feature of the evolve pipeline. Include:
- How a checkpoint is created before each task (Git HEAD SHA stored in `.greatsage/checkpoint_<n>.txt`).
- How the pipeline uses these checkpoints to resume after interruptions.
- Example commands to view checkpoints.

This informs users about the new safety feature and keeps documentation in sync with the code changes.
