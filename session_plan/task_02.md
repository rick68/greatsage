Title: Implement dry‑run execution mode for evolve pipeline
Files: src/evolve.rs, tests/evolve_dry_run.rs
Issue: none

Add a new public function `run_evolve_dry()` that performs only the assessment and planning phases, without executing any tasks.

1. In `src/evolve.rs`, create the function:
   ```rust
   pub fn run_evolve_dry() -> Result<(), Box<dyn std::error::Error>> {
       let assessment = assessment_phase(".")?;
       planning_phase_with_assessment(Path::new("."), &assessment)?;
       Ok(())
   }
   ```
   This mirrors the existing `run_evolve()` but skips `execute_tasks`.
2. Ensure the function is exported (pub) and included in the module’s public API.
3. Add a test in `tests/evolve_dry_run.rs` that:
   - Calls `run_evolve_dry()` on a temporary directory.
   - Verifies that the `session_plan/` directory and three task files are created.
   - Confirms that no placeholder marker files (`.greatsage/placeholder*.txt`) are generated.
4. Keep the test isolated using `tempfile::TempDir` and clean up after execution.
5. The test should compile and pass with the existing suite.

This task touches at most two source files and adds a new test file, staying within the per‑task file limit.
