Title: Add REPL error‑handling flag
Files: src/main.rs, src/cli.rs
Issue: none

Add a new command‑line flag `--error-handling` (or short `-e`) to the REPL. Update the CLI parsing in `src/main.rs` (or `src/cli.rs` if CLI is separate) to recognize this flag and store it in a configuration struct. Implement the flag so that the REPL validates input and, instead of panicking on errors, gracefully reports them and continues. This enhances stability and addresses the recurring gap identified in multiple assessments.

Steps:
1. Define a new field `error_handling: bool` in the configuration struct.
2. Extend the argument parser to accept `--error-handling`/`-e` and set the flag.
3. In the REPL loop, wrap prompt handling in a Result and, when the flag is enabled, catch errors and display a user‑friendly message rather than bubbling up a panic.
4. Add a unit test in `src/tests/` (or appropriate test module) that runs the REPL with the flag, triggers an error condition (e.g., malformed command), and asserts that the process exits cleanly with an error message.
5. Update documentation in `README.md` to document the new flag.
