Title: Implement REPL error‑handling flag
Files: src/main.rs, src/config.rs, src/cli.rs
Issue: none

Add a command‑line option (e.g., `--strict-errors` or `--error-handling`) that enables the REPL to validate required files before starting and to panic on missing files, matching the intended behavior described in earlier lessons. Update the configuration handling so the flag persists via the new `--check` mechanism and is respected on subsequent runs. Ensure the flag integrates with existing REPL loop and that appropriate error messages are displayed. Add unit tests for the flag's effect on REPL startup.
