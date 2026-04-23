Title: Token usage tracking and display in REPL/TUI
Files: src/agents/coding.rs, src/tui/mod.rs, src/main.rs
Issue: none

Implement live token usage accounting for LLM interactions and expose it in both REPL and TUI modes.

- In `src/agents/coding.rs` extend the LLM request/response handling to capture the number of tokens sent and received (use the LLM provider's response metadata if available; otherwise approximate by counting words/characters). Store these counts in a shared `Arc<Mutex<TokenStats>>` struct with fields `sent: usize` and `received: usize`.
- Add a method `fn get_stats(&self) -> TokenStats` to retrieve the current totals.
- In `src/tui/mod.rs` add a UI panel (e.g., bottom right) that renders "Tokens sent: X | received: Y" updating on each LLM turn.
- In `src/main.rs` (REPL path) after each LLM response, print the token stats to the console in a concise line.
- Write unit tests in `src/agents/coding.rs` (or a new test module) that simulate a mock LLM client returning known token counts and verify that `TokenStats` reflects the expected values after processing a request.
- Update README.md to document the new "Token usage" feature and how users can view it.

Ensure all existing tests continue to pass.