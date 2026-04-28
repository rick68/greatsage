Title: Implement --evolve flag to invoke evolution pipeline
Files:
- src/main.rs
Issue: none

## Description
Add functionality for the top‑level `--evolve` CLI flag (currently unused) to start the self‑evolution pipeline.

### Steps
1. In `src/main.rs`, after parsing `Args`, check `args.evolve`.
2. If true, invoke `evolve::run_evolve_dry()` for a dry‑run (assessment + planning) or `evolve::run_evolve()` for full run. Use a decision flag (e.g., also respect a new `--push` sub‑option if needed, but for now default to dry‑run).
3. Handle any errors by printing to stderr and exiting with non‑zero status.
4. Ensure the program returns after running evolve, bypassing REPL startup.
5. Add necessary import of `evolve` if not already in scope.

### Acceptance Criteria
- Running `cargo run -- --evolve` executes the evolve dry‑run without panicking.
- The command exits after completion, not starting the REPL.
- No existing behavior is broken; other flags still work.

### Tests
- Add a test that spawns the binary with `--evolve` and asserts it exits with code 0.
- Ensure the test runs quickly.

---
This task expands the usability of the evolve feature, making the flag functional and aligning with the project's roadmap.
