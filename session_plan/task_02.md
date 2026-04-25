Title: Add protected file verification in evolve pipeline
Files: src/evolve.rs
Issue: none

Implement a simple protection mechanism for the evolve pipeline. Add a constant array `PROTECTED_PATHS` containing the strings:
- ".github/workflows/"
- "IDENTITY.md"
- "scripts/"
- "skills/"
Create a helper function `fn verify_protected_paths(modified: &[std::path::PathBuf]) -> Result<(), String>` that iterates over the provided paths and returns an Err with a clear message if any path starts with one of the protected prefixes.
In `run_evolve()`, before proceeding with any logic, call this verification with an empty vector for now (placeholder for future modified files) and return the error if any.
This prepares the pipeline for future file‑modification checks without affecting current behavior.

