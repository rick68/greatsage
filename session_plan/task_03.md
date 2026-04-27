Title: Clean up placeholder task scaffolding in evolve.rs
Files: src/evolve.rs
Issue: none

**Description**
The evolve module currently contains multiple placeholder task comments and empty task functions that clutter the code and provide no functionality. This task will:
1. Remove all placeholder task stubs and related comments that do not contribute to the evolution pipeline.
2. Refactor the file to keep only the core skeleton (CLI integration, phase function signatures, protected‑path logic, and the new `run_evolution` entry point).
3. Ensure the module still compiles and all existing tests pass.
4. Update the module documentation comment at the top of `src/evolve.rs` to reflect the current state (only a skeleton and protected‑path enforcement).
5. Add a unit test in `tests/evolve_cleanup.rs` that verifies `run_evolution()` still returns `Ok(())` and that no placeholder functions are present (by checking that the binary output does not include placeholder phase names).

**Verification**
Run `cargo test` – new test must pass and all existing tests must still pass. Build the binary and execute `greatsage evolve` to confirm it runs without encountering undefined placeholders.
