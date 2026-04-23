Title: Refine PermissionConfig path validation and add tests
Files: src/agents/mod.rs, tests/permission_tests.rs, docs/src/permissions.md
Issue: none

Improve the permission sandbox validation to correctly distinguish file‑system paths from command‑line flags.
- In `PermissionConfig::validate_command` (src/agents/mod.rs) adjust the logic so that tokens starting with '-' are treated as flags, not paths, even if they contain '/'.
- Return a detailed error indicating which part of the command is disallowed rather than a generic message.
- Add unit tests in `tests/permission_tests.rs` covering:
  * valid commands with flags (e.g., `git -C /tmp status`)
  * disallowed paths outside the allowed root
  * mixed allowed and disallowed tokens
- Update documentation at `docs/src/permissions.md` to explain the new validation rules and examples.
