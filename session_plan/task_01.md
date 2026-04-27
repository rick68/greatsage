Title: Add global error‑handling flag to REPL
Files: src/main.rs, src/agents/coding.rs, README.md
Issue: none

Implement a new CLI flag `--error-handling` (or `--handle-errors`) that wraps the REPL execution in a top‑level panic catcher. When enabled, any panic inside the REPL should be caught, logged, and a user‑friendly message printed instead of crashing the process. Update the REPL loop in `src/agents/coding.rs` to return Result and propagate errors. Modify `src/main.rs` to parse the flag and invoke the new error‑handling wrapper. Add a short entry in `README.md` documenting the flag and its purpose. Ensure the change compiles and all existing tests still pass.
