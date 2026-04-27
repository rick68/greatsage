Title: Add dry‑run flag to Evolve subcommand
Files: src/cli.rs, src/main.rs, README.md
Issue: none

Implement a `--dry-run` (or `dry_run` boolean) option for the `evolve` subcommand.

1. In `src/cli.rs`, extend the `Command::Evolve` variant to include a `dry_run: bool` field with a Clap argument `#[arg(long, action = ArgAction::SetTrue)]`.
2. Update the CLI help text to mention the new flag.
3. In `src/main.rs`, when matching `Command::Evolve { dry_run }`, call a new function `evolve::run_evolve_dry()` if `dry_run` is true, otherwise call the existing `evolve::run_evolve()`.
4. Add a brief description of the `--dry-run` flag to `README.md` under the "Evolve" section, explaining that it performs assessment and planning phases only, without executing tasks.
5. Ensure that the binary still compiles and all existing tests pass.
