Title: Implement Phase A1 Assessment in evolve pipeline
Files: src/evolve.rs, src/cli.rs
Issue: none

Add the initial assessment phase (A1) to the evolve subcommand. The implementation should:
1. When `greatsage evolve` is invoked, run a lightweight assessment that:
   - Counts the number of Rust source files in `src/`.
   - Executes `cargo build` and captures success/failure.
   - Executes `cargo test` and captures success/failure.
2. Store the results in a simple struct `AssessmentResult` and print a concise summary to stdout.
3. Return an `Ok(())` result so the evolve flow can continue to the next phases.
4. Register the new behavior in `src/cli.rs` under the `evolve` subcommand (or flag) and ensure the binary still builds.
5. Add a brief entry to `README.md` documenting that the evolve command now performs an assessment step.

The change must compile and all existing tests must continue to pass.
