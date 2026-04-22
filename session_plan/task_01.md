Title: Enforce PermissionConfig in file system tools
Files: src/agents/coding.rs, src/agents/mod.rs
Issue: none

Add permission checks for all file‑system‑related tools (read_file, write_file, edit_file, list_files, search, bash). Implement a helper in `src/agents/mod.rs` that evaluates a given path against the `PermissionConfig` whitelist/blacklist and returns an error if disallowed. In `src/agents/coding.rs` where tool calls are handled, invoke this check before performing the operation and surface a clear error message to the user if the path is prohibited. Update or add unit tests confirming that disallowed paths are blocked while allowed paths succeed.
