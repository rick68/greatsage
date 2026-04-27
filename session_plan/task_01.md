Title: Implement REPL error handling flag for interactive mode
Files: src/main.rs
Issue: none

Implement the REPL error‑handling validation for interactive sessions. When the `--error-handling` (or persisted `--repl-error-handling`) flag is enabled, each line entered in the REPL should be passed through `handle_prompt` to validate file references before being sent to the coding agent. Update the REPL loop in `src/main.rs` to invoke `handle_prompt` for each input, handling any returned error by printing a message and discarding the prompt. Ensure the flag is respected both for one‑shot prompts and continuous REPL usage.

