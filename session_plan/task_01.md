Title: Add missing REPL error‑handling flag
Files: src/main.rs, src/cli.rs
Issue: none

Implement a `--error-handling` flag for the REPL. Update the CLI definition to include the flag, adjust the REPL startup to enable guardrails when the flag is present, and add appropriate help text. Ensure the flag toggles the existing permission‑error guard (currently missing). Update documentation in `README.md` to mention the new flag. Add a unit test verifying that invoking the binary with `--error-handling` sets the internal configuration without panicking.