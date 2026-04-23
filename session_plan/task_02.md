Title: Robust tool error handling for Bash tool
Files: src/agents/coding.rs, src/agents/mod.rs
Issue: none

Add structured error handling for tool execution, focusing on the Bash tool.

- In `src/agents/mod.rs` introduce a new enum `ToolError` with variants such as `ExecutionFailed(String)` and `NonZeroExit(i32, String)`. Implement `std::fmt::Display` and `std::error::Error` for it.
- In `src/agents/coding.rs` locate the code that spawns the bash command (likely using `std::process::Command`). Change the logic to capture `stdout`, `stderr`, and the exit status.
- If the command exits with a non‑zero status, return a `ToolError::NonZeroExit(code, stderr)` wrapped in a `Result`.
- Propagate this error up to the LLM response layer so that the agent can surface a clear diagnostic message like "Bash command failed with exit code 2: <stderr>" instead of silently ignoring it.
- Add unit tests in `src/agents/mod.rs` (or a new test module) that invoke a deliberately failing Bash command (e.g., `false` or `exit 1`) and assert that the returned error matches `ToolError::NonZeroExit` with the correct code and message.
- Ensure the existing tests still pass.

Documentation: update `README.md` section "Tool usage" to mention that Bash tool now reports detailed errors.
