Title: Improve missing environment variable error messages
Files:
- src/main.rs
Issue: none

Description:
Enhance the error output when required environment variables are missing.

1. Locate the code in `src/main.rs` that checks for required env vars and exits with a message.
2. Modify it to list all missing variables in a single, clear error message, e.g., "Error: Missing required environment variables: VAR1, VAR2".
3. Ensure the message is printed to stderr and the process exits with a non-zero code.
4. Add a unit test in `src/main.rs` (or a new test module) that simulates missing env vars and asserts the error string contains all missing names.
5. Verify `cargo build` and `cargo test` succeed.
