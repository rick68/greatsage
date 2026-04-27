Title: Detect CI workflow presence in assessment phase
Files: src/evolve.rs, src/tests/assessment_ci_status.rs
Issue: none

Implement CI status detection in `assessment_phase`:
- Scan the `.github/workflows` directory for any `*.yml` or `*.yaml` files.
- If at least one workflow file is found, set `ci_status` to "present"; otherwise set it to "none".
- Update the formatted output string to include the CI status.
- Add a unit test `assessment_ci_status` covering both scenarios using temporary directories.
- Ensure the function still respects the existing timeout and returns an error on timeout.
