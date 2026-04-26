Title: Add robust error‑handling flag to REPL
Files: src/main.rs, src/cli.rs, src/config.rs
Issue: none

## Description
Implement a new command‑line flag (e.g., `--error‑on‑panic` or `--strict`) that enables strict error handling in the REPL loop. When the flag is set, any internal error should cause the REPL to exit with a non‑zero status instead of attempting to continue. This addresses the biggest capability gap identified in the assessment.

### Steps
1. **src/cli.rs** – Extend the argument parser to recognize the new flag (e.g., `--strict-errors`). Add a field in the CLI config struct.
2. **src/config.rs** – Add a boolean configuration option `strict_errors` with default `false`. Ensure it can be set via CLI flag.
3. **src/main.rs** – In the REPL loop, check the `strict_errors` config. Wrap core REPL handling in a `match` that, on `Err(_)`, either logs and continues (default) or exits with `std::process::exit(1)` when strict mode is enabled.
4. Add unit tests in `src/tests/` (or existing test module) verifying that with `strict_errors` enabled, a simulated error causes the process to return error status. Use a mock or injection to trigger an error.
5. Update documentation (README.md) to mention the new flag.

### Acceptance Criteria
- The binary compiles (`cargo build`).
- All existing tests still pass.
- New tests for the flag pass.
- Running `greatsage --strict-errors` with an induced error exits with code 1.
- Documentation reflects the flag.
