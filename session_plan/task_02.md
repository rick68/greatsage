Title: Add comprehensive REPL error handling guardrails
Files: src/main.rs
Issue: none

Enhance REPL robustness by extending the `handle_prompt` error handling. Introduce a new function `validate_prompt_file` that checks for file existence, readability, and size limits, returning detailed errors. Integrate this validation into `handle_prompt` when `repl_error_handling` is enabled. Update relevant CLI flags documentation in `src/cli.rs` to describe the new validation behavior. Add unit tests in `tests/repl_error_handling.rs` to cover successful validation, missing file, and unreadable file scenarios. Ensure no changes to other modules and keep modifications limited to at most three files.
