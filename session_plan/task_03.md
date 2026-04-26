Title: Update documentation for experimental error-handling flag
Files: README.md, src/cli.rs
Issue: none

## Description
The new `--error-handling` flag now affects REPL prompt validation. Documentation is currently missing:

- Add a section in `README.md` describing the flag, its purpose (experimental file validation), and how it differs from `--strict-errors`.
- Update the CLI help output in `src/cli.rs` to include a short description for the flag (the Arg already exists but help text can be clarified).

### Goals
1. Insert a bullet point under "Command‑line options" in `README.md`.
2. Ensure the `#[arg]` annotation for `error_handling` in `src/cli.rs` includes a concise description.
3. Run tests to confirm no breakage.

### Acceptance Criteria
- The README shows the flag with a clear explanation.
- `greatsage --help` displays the updated description.
- All existing tests pass (`cargo test`).
