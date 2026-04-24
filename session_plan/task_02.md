Title: Refine bash command permission validation
Files: src/agents/mod.rs
Issue: none

## Description
Improve `PermissionConfig::validate_command` to avoid false‑positives.

- Skip tokens that look like URLs (contain "://").
- Skip JSON literals (tokens starting with `{` or `[`).
- Continue to skip flag tokens (`-...`).
- For remaining tokens that contain a `/` or start with `.` treat them as file paths and validate via `validate_path`.
- Update documentation/comments to explain the new heuristic.
- Add unit tests covering URL, JSON, flag, and path cases to ensure correct behavior.

## Documentation
Update any relevant developer docs to reflect the refined validation logic.
