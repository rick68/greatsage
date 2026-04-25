Title: Add protected path verification to evolve pipeline
Files: src/evolve.rs
Issue: none

Implement a helper function `is_protected_path(path: &Path) -> bool` that returns true for paths within `.github/workflows/`, `IDENTITY.md`, `scripts/`, and `skills/`. Integrate this check into `planning_phase` to ensure generated task files are not placed in protected locations. If a protected path is detected, return an error to abort planning.

Update tests if needed to verify that attempting to create a task in a protected directory fails.
