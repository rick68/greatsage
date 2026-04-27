Title: Add REPL error‑handling flag
Files: src/main.rs, src/cli.rs
Issue: none

Implement a new command‑line flag `--error-handling` for the REPL. When enabled, the REPL should gracefully handle missing input files by printing a clear error message and exiting with a non‑zero status instead of panicking. Update the argument parser in `src/cli.rs` to expose the flag, and adjust the REPL startup logic in `src/main.rs` to check the flag and perform the guardrail. Ensure the flag defaults to off to preserve current behavior.

Also add documentation for the flag in the generated help output (already handled by clap).
