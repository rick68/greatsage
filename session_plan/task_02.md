Title: Add "stats" subcommand to display assessment information
Files: src/cli.rs, src/main.rs, src/tests/cli_stats.rs
Issue: none

## Description
Provide a quick way for users to see basic project statistics (version, source file count, CI status) without entering full evolve mode.

1. Extend `Command` enum in `src/cli.rs` with a new variant `Stats`.
2. Update Clap metadata to include a help description.
3. In `src/main.rs`, after parsing args, detect `Command::Stats` and invoke `evolve::assessment_phase` (or a helper) to obtain the assessment string and print it to stdout, then exit with code 0.
4. Add a unit test `src/tests/cli_stats.rs` that constructs an `Args` with the `stats` subcommand, runs the handling logic (you can call `evolve::assessment_phase` directly) and asserts the output contains the version line.
5. Ensure existing functionality (including the new `--error-handling` flag) remains unchanged.

## Acceptance Criteria
- New subcommand `stats` is recognized (`greatsage --stats` or `greatsage stats`).
- Running it prints a multi‑line string with at least `Version:` and `Source files:`.
- Unit test verifies the command works.
- Build and all tests pass.
