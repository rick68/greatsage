Title: Add REPL error‑handling configuration flag
Files: src/config.rs, src/cli.rs
Issue: none

## Description
Introduce a new configuration option that controls how the REPL handles missing files.

- In `src/config.rs` add a boolean field `allow_missing_files` (default `false`).
- Expose this option via the CLI (e.g., `--allow-missing-files` flag) in `src/cli.rs` and map it to the config field.
- Update the configuration loading logic to set this flag based on the CLI argument.
- Ensure the flag is documented in the README under a new "REPL Options" section (optional for later).

The change must compile and not affect existing behavior when the flag is omitted.
