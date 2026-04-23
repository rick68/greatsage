Title: Enforce permission checks for bash tool
Files: src/agents/coding.rs, src/agents/mod.rs
Issue: none

Modify the REPL agent to apply the existing `PermissionConfig` when executing the `bash` tool. Before running any command, verify that the command’s target paths (if any) are allowed by the whitelist. If disallowed, return a clear error message to the LLM instead of executing the command. Update the tool registration in `agents::mod` if needed to expose the permission config to the coding agent.

Ensure the changes compile and all tests still pass.
