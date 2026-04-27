Title: Implement REPL `--error-handling` flag
Files: src/main.rs, src/config.rs, src/cli.rs
Issue: none

Description:
Implement the missing `--error-handling` command‑line flag for the REPL. The flag should enable strict validation of required configuration files before the REPL starts. When enabled, the program must:
1. Parse the flag via the existing CLI argument parser.
2. Invoke a validation routine (reuse the `--check` logic already present) that verifies required files exist and are readable.
3. If any validation fails, exit with a non‑zero status and emit a clear error message.
4. When the flag is not set, retain current permissive behavior.

Implementation steps:
- Add a boolean field `error_handling` to the CLI options struct in `src/cli.rs` and wire the flag `--error-handling`.
- Extend the argument parsing in `src/main.rs` to capture the new flag.
- In the REPL initialization path, after configuration loading, invoke `config.validate()` (or equivalent) when `error_handling` is true; abort on failure.
- Update `src/config.rs` if needed to expose a validation API that is used by both `--check` and the new flag.
- Add unit tests in `tests/` to ensure the flag causes the process to exit with error on missing files and passes when all files are present.
- Update README to document the new flag.

Goal: Resolve the persistent “missing error‑handling flag” gap identified in the assessment.
