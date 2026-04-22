Title: Add basic permission check for file write operations
Files:
- src/agents/coding.rs
- src/agents/mod.rs
Issue: none

Description:
Introduce a simple permission system that restricts file write operations to a configurable whitelist directory.

1. In `src/agents/mod.rs` add a new struct `PermissionConfig` with a field `allowed_dir: PathBuf` (default to the current working directory). Provide a function `is_path_allowed(&self, path: &Path) -> bool` that checks if the given path starts with `allowed_dir`.
2. Expose this config as a resource in the Bevy app (similar to other resources) so the coding agent can access it.
3. In `src/agents/coding.rs` modify the handling of the "write_file" and "edit_file" tools: before performing the operation, retrieve the `PermissionConfig` resource and verify the target path is allowed using `is_path_allowed`. If not, return an error string like "Permission denied: path not within allowed directory.".
4. Add unit tests for the permission check: create a temporary directory, set it as `allowed_dir`, attempt to write to a file inside (should succeed) and outside (should be denied).
5. Ensure the existing tests still pass and the new tests are included in `cargo test`.

Update any relevant documentation (e.g., YOYO.md) to mention the new permission feature.
