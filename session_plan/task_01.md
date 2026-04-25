Title: Add REPL error‑handling flag and validation
Files: src/cli.rs, src/main.rs, src/tests/repl_error_handling.rs
Issue: none

## Description
The REPL currently has no way to validate prompts or guard against common user errors (e.g., empty input, missing files, malformed commands). Competitors provide an explicit error‑handling flag. Implement a new CLI flag `--error‑handling` (bool) that, when enabled, makes `handle_prompt` perform additional checks:

1. Reject empty prompts (already a no‑op, but keep explicit).
2. If the prompt looks like a file path (e.g., starts with `file:` or ends with a known extension like `.rs`, `.txt`), verify the file exists and is readable; otherwise return an error.
3. Propagate any validation error as `Err(Box<dyn Error>)`.

Update `Args` in `src/cli.rs` to include the flag, defaulting to `false`. In `main.rs`, modify `handle_prompt` to perform the new checks when the flag is true. Add a unit test file `src/tests/repl_error_handling.rs` covering:
- Flag disabled → `handle_prompt` accepts any non‑empty string.
- Flag enabled + existing file → succeeds.
- Flag enabled + non‑existent file → returns an error.

The test should create a temporary file in the test’s temp directory, invoke `handle_prompt` with `--error‑handling` via constructing an `Args` instance or by directly calling the function with the flag set in a temporary `AppConfig`.

## Acceptance Criteria
- New CLI flag `--error‑handling` is parsed and stored in `AppConfig.runtime` (add a field `error_handling: bool`).
- `handle_prompt` returns an error when validation fails and the flag is enabled.
- All existing tests continue to pass.
- New tests for the flag pass.
- `cargo build`, `cargo test`, and `cargo clippy` succeed.
