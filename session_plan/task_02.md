Title: Add Git commit and tag after successful evolve run
Files: src/evolve.rs, src/git.rs, src/lib.rs
Issue: none

## Description
After a successful evolution cycle, record the changes in Git and create a tag for the iteration. This provides traceability and aligns with the full evolve pipeline.

### Steps
1. **src/lib.rs** – Add a helper `fn current_iteration() -> Result<String>` that reads the iteration number from the `ITERATION_COUNT` file (or a new `iteration.txt`).
2. **src/git.rs** – Add a function `fn commit_and_tag(iteration: &str) -> Result<()>` that runs:
   - `git add -A`
   - `git commit -m "evolve iteration {iteration}"`
   - `git tag v{iteration}`
   - `git push && git push --tags`
   Propagate any errors.
3. **src/evolve.rs** – After all tasks finish successfully, call `current_iteration()` and then `commit_and_tag(&iteration)`.
4. Add a unit test (or integration test) that creates a temporary git repository, modifies a file, calls the new functions, and asserts that a commit and tag exist. Use the `git2` crate or invoke the git binary in the test.
5. Ensure the `Cargo.toml` includes any new dependencies (e.g., `git2`).

The change touches exactly three source files and can be verified with `cargo test` and a manual run of `--evolve` in a test repo.
