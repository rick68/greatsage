Title: Add unit tests for REPL error‑handling flag
Files: src/main.rs, tests/error_handling_test.rs
Issue: none

Create integration tests verifying that when the `--error-handling` flag is passed and the REPL is started with a missing input file, the program exits with a non‑zero status and prints an informative error message. Also test that without the flag the previous panic behavior (or default) remains unchanged. Add the test file under `tests/` and ensure it runs with `cargo test`. Update Cargo.toml if needed to include the test as an integration test.

Document the test expectations in comments.
