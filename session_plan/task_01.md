Title: Implement REPL error‑handling flag
Files: src/main.rs, src/config.rs
Issue: none

Add a configurable error‑handling flag for the REPL that, when enabled, validates all file‑related prompts (e.g., write, read, delete) and aborts the operation if the target path is outside allowed directories or otherwise unsafe. Extend `Config` in `src/config.rs` with a boolean `repl_error_handling` (default false) and expose a CLI option `--repl-error-handling` (or an env var). In `src/main.rs`’s REPL loop, check this flag before executing file operations and return a clear error message without performing the action. Update REPL help text accordingly.
