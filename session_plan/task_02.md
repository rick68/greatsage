Title: Integrate Git commit after successful evolve run
Files: src/evolve.rs
Issue: none

Update the evolve pipeline to automatically commit changes upon successful completion of all tasks. Import the new `commit_changes` function from `src/git.rs` and invoke it after `execute_tasks` succeeds. Handle any errors by logging them to `evolve.log` but do not abort the pipeline. This adds basic Git integration for the evolve workflow.
