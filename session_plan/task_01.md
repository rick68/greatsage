Title: Add comprehensive REPL error handling
Files: src/main.rs, src/cli.rs, src/repl.rs (if exists)
Issue: none

**Description**
The current REPL lacks robust error handling and only provides a simple `--check` flag for file existence. This task will:
1. Introduce a new command‑line flag `--error‑handling` (or enable by default) that wraps the REPL main loop in a `std::panic::catch_unwind` block.
2. On panic, capture the error, print a user‑friendly message, and exit with a non‑zero status without crashing the whole binary.
3. Add a custom error type `ReplError` for common failure modes (missing config, IO errors) and ensure all REPL entry points return `Result<(), ReplError>`.
4. Update the REPL entry point in `src/main.rs` to propagate errors and use the new error handling.
5. Add unit tests in `tests/repl_error_handling.rs` verifying that a simulated panic inside the REPL is caught and results in a graceful shutdown (exit code 1) and that the error message contains "REPL encountered an unexpected error".

**Documentation**
Update `README.md` to describe the new error handling behavior and the `--error‑handling` flag.

**Verification**
Run `cargo test` – new tests must pass. Run the binary with a forced panic (e.g., environment variable `FORCE_PANIC=1`) and verify graceful exit.
