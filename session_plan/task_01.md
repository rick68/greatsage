Title: Add git commit and tag step to evolve pipeline
Files: src/evolve.rs, src/git.rs, src/cli.rs
Issue: none

Implement automatic git commit, tag creation, and push at the end of a successful evolve run.

- In `src/evolve.rs`, after all tasks complete successfully, invoke functions from `src/git.rs` to stage changes, create a commit with a message like "evolve iteration ${ITERATION}", create an annotated tag `v${ITERATION}`, and push both commit and tag.
- Add a new CLI option `--push` (default false) to the evolve subcommand in `src/cli.rs` that enables pushing; if omitted, only local commit/tag are created.
- Update `src/git.rs` with helper functions `commit_and_tag(iteration: u32, push: bool) -> Result<(), GitError>` that wraps existing git helpers.
- Ensure errors are propagated and cause the evolve run to fail gracefully.
- Add a basic unit test in `src/tests/evolve_cli.rs` to verify that the `--push` flag is parsed correctly (already exists; ensure no regression).

Documentation:
- Update `README.md` section for the `--evolve` flag to mention the new commit/tag behavior and the optional `--push` flag.
