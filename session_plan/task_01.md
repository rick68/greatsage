Title: Implement REPL error-handling flag
Files: src/main.rs
Issue: none

Add actual functionality for the `--error-handling` flag referenced in the REPL. Implement a command‑line flag that, when enabled, wraps the REPL loop in a panic catcher and prints a friendly error message instead of crashing. Update the argument parsing to store the flag and modify the REPL execution path to respect it. Ensure the flag defaults to disabled to preserve current behavior.
