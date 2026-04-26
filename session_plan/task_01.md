Title: Integrate experimental error-handling flag into REPL workflow
Files: src/config.rs, src/main.rs, src/cli.rs
Issue: none

## Description
The CLI provides an `--error-handling` flag (experimental) but the runtime does not currently use it. Instead, REPL validation uses the `strict_errors` flag, which is intended for panic‑hook behavior only. This disconnect means the experimental flag has no effect, and users cannot enable file‑validation without also enabling strict‑error exit behavior.

### Goals
1. Add a boolean `error_handling` field to `RuntimeConfig`.
2. Populate it from the `--error-handling` CLI argument.
3. In `main.rs`, derive `error_handling_flag` from `app_config.runtime.error_handling` (separate from `strict_errors`).
4. Ensure `handle_prompt` receives the correct flag, preserving existing tests.
5. Update documentation in `README.md` (or CLI help) to mention the new flag behavior.

### Acceptance Criteria
- The `--error-handling` flag can be passed on the command line and influences prompt validation.
- Existing tests (`repl_error_handling.rs`) continue to pass.
- Running `greatsage --error-handling` with an invalid file path returns an error instead of silently succeeding.
- The strict‑errors panic hook remains controlled by `--strict-errors`.

### Notes
- No changes to the behavior of `--strict-errors` are required.
- Ensure backward compatibility: if neither flag is set, default behavior stays unchanged.
