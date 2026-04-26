Title: Implement proper REPL error‑handling guardrail
Files: src/main.rs, src/config.rs
Issue: none

Add a concrete error‑handling flag for the REPL that actually catches panics or user errors and prevents the REPL from crashing. Steps:
1. In `src/config.rs` define a new `bool` field `repl_error_handling` with a default `true` and expose it via CLI config parsing.
2. In `src/main.rs` where the REPL loop reads user input and invokes the LLM, wrap the call in a `std::panic::catch_unwind` block. If a panic occurs, log the error, print a friendly message, and continue the REPL.
3. Ensure any I/O errors from reading stdin are also handled gracefully.
4. Add unit tests in `tests/repl_error_handling.rs` verifying that a simulated panic inside the LLM call does not crash the REPL and that the flag can be toggled off to allow the panic to propagate (for debugging).
5. Update README section on REPL usage to mention the new `--repl-error-handling` flag.

This addresses the persistent missing REPL error‑handling gap identified in the assessment.
