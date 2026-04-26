Title: Add CI status integration to assessment phase
Files: src/evolve.rs, src/config.rs
Issue: none

Implement a way for the assessment phase to report actual CI status instead of the placeholder "unknown".
Steps:
1. In `src/config.rs` add an optional field `ci_status_source` (enum) with values `EnvVar` (default) and `GitHubActions`. Provide CLI flag `--ci-status-source`.
2. In `src/evolve.rs` modify `assessment_phase()` to read this config. If `EnvVar`, read `CI_STATUS` env var; if `GitHubActions`, attempt to query the GitHub Actions API for the latest workflow run status for the repository (use `gh` CLI if available, fallback to env var). Populate the assessment output with the retrieved status.
3. Add a unit test `tests/ci_status_integration.rs` that sets `CI_STATUS` env var and verifies `assessment_phase()` returns the expected status string.
4. Update `README.md` section on `--ci-status-source` flag.

This replaces the placeholder CI status and provides useful information for future evolution decisions.
