Title: Implement Phase A1 Assessment in evolve pipeline
Files: src/evolve.rs, src/agents/coding.rs, src/cli.rs
Issue: none

## Description
Implement the assessment (Phase A1) of the self‑evolution pipeline.
- Add a function `run_assessment` in `src/evolve.rs` that collects basic codebase metrics (file count, lines of code, recent commits) and writes an assessment markdown file.
- Use a timeout of half of the total evolution timeout (`TIMEOUT/2`).
- Integrate with the REPL/CLI so that the `--evolve` flag triggers this assessment before any other phase.
- Ensure the function returns a structured `AssessmentResult` that can be used by the planning phase.
- Add minimal unit tests in `src/evolve.rs` verifying that the assessment creates a non‑empty report and respects the timeout.
- Update `src/cli.rs` to expose the new subcommand or flag handling, wiring it to call `run_assessment`.
- Document the new behavior in `README.md` under a "Evolution Pipeline" section.

### Acceptance Criteria
- `cargo build && cargo test` passes.
- Running `greatsage --evolve` (or `cargo run -- --evolve`) prints the assessment summary.
- Assessment report file `assessment.md` is created in the project root.
- Tests confirm report creation and timeout enforcement.
- README updated with a brief description of the `--evolve` flag and Phase A1.
