Title: Enhance assessment phase CI status detection
Files: src/evolve.rs, src/tests/assessment_ci.rs
Issue: none

Add logic to `assessment_phase` to check for any files under `.github/workflows`.
If at least one workflow file exists, set `ci_status` to "found", otherwise "none".
Create a new test `src/tests/assessment_ci.rs` that creates a temporary directory with a mock workflow file and verifies the CI status string contains "found" when present and "none" when absent.

Update any necessary imports.
