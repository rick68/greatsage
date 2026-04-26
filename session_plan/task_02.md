Title: Add tests for REPL error-handling flag
Files: src/main.rs, src/tests/error_handling_flag.rs
Issue: none

Create a new integration test verifying that when the `--error-handling` CLI flag is enabled, `handle_prompt` validates file‑like prompts and rejects nonexistent files, while passing through normal text. The test should:
1. Build a temporary file with a known extension (e.g., .txt) and ensure `handle_prompt` returns Ok.
2. Pass a nonexistent filename and assert an error.
3. Pass a non‑file string (e.g., "hello") and assert success.
The test file `src/tests/error_handling_flag.rs` should use the existing `handle_prompt` function and set the flag to true. No changes to production logic beyond exposing the flag (handled in Task 1).