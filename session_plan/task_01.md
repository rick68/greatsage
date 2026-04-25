Title: Add robust error‑handling flag for REPL prompts
Files: src/main.rs, src/agents/coding.rs
Issue: none

Implement a comprehensive error‑handling flag for the REPL that validates prompts before execution. Steps:
1. In `src/main.rs`, extend the CLI to include a `--error-check` (or similar) flag that toggles validation.
2. In `src/agents/coding.rs`, add a function `validate_prompt(&self, prompt: &str) -> Result<(), String>` that checks for common issues (non‑empty, no disallowed characters, optional file‑existence checks for file‑based prompts).
3. Integrate the validation into the REPL loop so that prompts are rejected early with a clear error message when the flag is enabled.
4. Update the REPL output handling to display validation errors in red text.
5. Add unit tests for the new validation function covering normal, empty, and malformed prompts.
6. Ensure `cargo build` and `cargo test` pass.

Documentation updates: add a brief description of the new flag to the README under the REPL usage section.
