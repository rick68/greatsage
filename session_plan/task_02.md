Title: Add Robust Error Handling to REPL Input Loop
Files: src/main.rs
Issue: none

Improve the REPL robustness by wrapping the input handling and agent execution in error‑handling logic to prevent crashes from unexpected input or runtime errors.

Steps:
1. Locate the REPL initialization code around lines 190‑210 where the TUI plugin is added and the optional invocation_prompt is processed.
2. Introduce a helper function `handle_prompt(prompt: String) -> Result<(), Box<dyn std::error::Error>>` that encapsulates the existing prompt‑sending logic.
3. In the `if let Some(prompt) = invocation_prompt` block, call this helper and on error log the issue with `eprintln!` and continue gracefully instead of exiting the app.
4. Ensure any panics in the `CodingAgentPromptChannel` send are caught using `std::panic::catch_unwind` and converted into an error.
5. Update imports as needed (`use std::error::Error;` etc.).
6. Add a unit test in `src/main.rs` (or a new test module) verifying that an empty or malformed prompt does not cause a panic – simulate by calling `handle_prompt("".to_string())` and asserting it returns `Ok(())`.

The change must touch at most 3 files (main.rs and its test module). Ensure `cargo test` passes after the modification.
