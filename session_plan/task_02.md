Title: Integrate error‑handling flag into REPL execution
Files: src/main.rs, src/config.rs, src/cli.rs
Issue: none

## Description
Update the REPL core to respect the new `allow_missing_files` configuration option.

1. In `src/main.rs` locate the code path that loads files for commands (e.g., `load_file`, `read_file`).
2. Wrap the file‑loading logic:
   - If the file exists, proceed as before.
   - If the file does not exist:
     * When `config.allow_missing_files` is `false` (default), return an error that propagates to the REPL and is displayed to the user.
     * When `true`, silently ignore the missing file and continue execution (or return an empty string as appropriate).
3. Ensure the error is propagated using the existing error handling mechanism (Result/anyhow) without panicking.
4. Add a unit test in `tests/` (e.g., `tests/repl_missing_file.rs`) that verifies both behaviours based on the flag.
5. Update any documentation comments in `src/main.rs` that reference the previous behaviour.

The change must compile, pass existing tests, and the new test must also pass.
