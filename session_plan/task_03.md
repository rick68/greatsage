Title: Add subcommand `evolve` for evolve mode
Files: src/cli.rs, src/main.rs, README.md
Issue: none

Implement a dedicated CLI subcommand `evolve` (instead of the `--evolve` flag) to trigger the evolution pipeline, aligning with typical tool conventions and improving discoverability.

Steps:
1. In `src/cli.rs` add a new enum variant `Evolve` to `Command` with `#[command(about = "Run the self‑evolution pipeline")]` and no extra args.
2. In `src/main.rs` extend the match after parsing `args.command` to handle `Command::Evolve` similarly to the existing `if args.evolve` block: call `evolve::run_evolve()` and exit on success or error.
3. Deprecate the `--evolve` flag by keeping it for backward compatibility but prefer the subcommand; optionally emit a warning when used.
4. Update `README.md` (or usage docs) to document the new `greatsage evolve` command with examples.
5. Add a unit test in `src/tests/evolve_cli.rs` that parses `Args` with `evolve` subcommand and ensures `args.command` matches `Command::Evolve`.

All changes must compile and existing tests must pass.
