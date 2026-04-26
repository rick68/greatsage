Title: Add unit tests for REPL error‑handling flag
Files: src/main.rs, tests/repl_tests.rs
Issue: none

Create a new integration test module `tests/repl_tests.rs` that spawns the REPL with the `--repl-error-handling` flag enabled. Verify that attempting to write to a disallowed path (e.g., "../outside.txt") results in an error message and no file created. Also test that a safe path within the allowed directory succeeds. Use the existing `Config` struct to enable the flag programmatically if needed. Ensure tests run under `cargo test` and pass when the flag works correctly.
