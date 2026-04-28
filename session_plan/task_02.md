Title: Extend protected paths to include .greatsage directory
Files: src/evolve.rs
Issue: none

## Description
Update `is_protected_path` function to treat the `.greatsage` directory as protected, preventing evolve pipeline from creating or modifying task files inside it. This guards against accidental self‑modification of internal state.

Steps:
1. Add `Path::new(".greatsage")` to the `protected` array.
2. Adjust any documentation/comments accordingly.
3. Update or add tests in `src/tests/evolve_protection.rs` to verify that paths under `.greatsage` are considered protected and that `execute_tasks` aborts if a task file is placed there.
