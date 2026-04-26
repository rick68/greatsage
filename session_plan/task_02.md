Title: Scaffold checkpoint‑restart logic for evolve pipeline
Files: src/evolve.rs, src/git.rs
Issue: none

## Description
Add a minimal scaffolding for checkpoint‑restart in the evolve subcommand. The implementation will:
1. Detect if a previous evolve run was interrupted by checking for a temporary checkpoint file (e.g., `.greatsage/evolve_checkpoint.json`).
2. If found, load the stored Git state (branch, HEAD hash) using functions in `src/git.rs`.
3. Provide a function `evolve::resume_checkpoint()` that returns an `Option<EvolveState>`.
4. In `evolve::run_evolve()`, call `resume_checkpoint()` at start and log whether a resume will occur.
5. Add a unit test that simulates creating a checkpoint file and ensures `resume_checkpoint()` returns the expected state.

The actual retry/re-execution logic will be implemented later; this scaffold satisfies the requirement of having checkpoint‑restart capability in place.

## Acceptance Criteria
- Code compiles (`cargo build`).
- Existing tests pass.
- New test verifies checkpoint detection.
- No external side effects; uses only filesystem.
