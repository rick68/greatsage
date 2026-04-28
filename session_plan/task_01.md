Title: Persist REPL error handling flag to config
Files:
- src/main.rs
- src/config.rs
Issue: none

## Description
Add functionality so that when the user enables REPL error handling via any of the CLI flags (`--check`, `--error-handling`, `--handle-errors`, `--repl-error-handling`), the `repl_error_handling` field in the persisted `AppConfig` is updated on disk.

### Steps
1. In `src/main.rs`, after parsing CLI arguments and determining the final `app_config.repl_error_handling` value, compare it with the loaded config's original value.
2. If they differ, call `app_config.save(&config_path)` to write the updated config back to the file.
3. Ensure `save` method exists (it already does) and returns a `Result`.
4. Add a small helper function `maybe_save_repl_error_handling` to keep `main` tidy.
5. Update imports accordingly.

### Acceptance Criteria
- Running `cargo run -- --check` updates the config file's `repl_error_handling` to `true`.
- Running without any flag leaves the value unchanged.
- No existing functionality is broken; all tests continue to pass.

### Tests
- Add a test that creates a temporary config file, runs the binary with `--check` using `std::process::Command`, and asserts the file now contains `repl_error_handling = true`.
- Ensure the test runs quickly and cleans up the temp file.

### Documentation
- Update `README.md` to mention the persisted flag behavior.

---
This task addresses the recurring gap reported in assessments: the missing persistent error‑handling flag for the REPL.
