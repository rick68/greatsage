Title: Implement basic evolve subcommand skeleton
Files: src/main.rs, src/evolve.rs
Issue: none

**Description**
Add a proper CLI subcommand `evolve` (or `--evolve` flag) that initiates the self‑evolution pipeline.
1. In `src/main.rs`, extend the argument parser to recognize `evolve` as a subcommand (using clap or existing parser).
2. When invoked, call a new public function `run_evolution()` defined in `src/evolve.rs`.
3. Implement `run_evolution()` to print a clear start message (e.g., "Starting evolution pipeline...") and sequentially call placeholder functions for phases A1, A2, B, C, wrapping them in a simple flow.
4. Each phase function should currently just log its name and return `Ok(())`.
5. Ensure the command returns a proper exit code (0 on success, non‑zero on any error).
6. Add a unit test in `tests/evolve.rs` that runs `run_evolution()` and asserts that the output contains the start message and all phase names.

**Documentation**
Update `README.md` to mention the new `evolve` subcommand and its purpose.

**Verification**
Run `cargo test` – new test must pass. Build the binary and execute `greatsage evolve` – should exit with code 0 and print the start message.
