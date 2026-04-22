Title: Add robust error handling for LLM request failures
Files: src/agents/coding.rs
Issue: none

Modify the `spawn_agent_task` function to handle potential errors from the LLM provider.
- Change the call to `coding_agent.lock().await.prompt(prompt.clone()).await` to capture a `Result<UnboundedReceiver<AgentEvent>, AgentError>` (or appropriate error type). If it returns an Err, log a clear error message to the TUI output (using `CodingAgentTask`'s output vector) and set the agent state back to `Idle` without panicking.
- Ensure the UI shows the error to the user and that the application continues running.
- Add any necessary `use` statements for the error type.
- Keep the existing logic for successful streams unchanged.

This improves stability by preventing crashes when the LLM API is unavailable or returns an error.
