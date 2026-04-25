Title: Integrate error‑handling flag into REPL input processing
Files: src/main.rs, src/agents/coding.rs
Issue: none

The `--error-handling` CLI flag currently only affects the initial prompt passed via `handle_prompt`. Users expect the same validation for any file reference entered during the REPL session. Modify the REPL handling code (in `agents/coding.rs`) to propagate the runtime `error_handling` setting to each call of `handle_prompt` when processing user input. Ensure the flag is read from `app_config.runtime.error_handling` and passed accordingly.

Update documentation in the README to mention that the flag now applies to REPL inputs as well. Add a test in `src/tests/repl_error_handling.rs` verifying that when the flag is enabled, entering a non‑existent file path via the REPL results in an error.
