Title: Expand tests for protected‑path detection
Files: tests/evolve_protection.rs
Issue: none

Add additional test cases to `tests/evolve_protection.rs` covering edge cases such as paths with symlinks, absolute paths, and hidden directories that should not be considered protected. Ensure `is_protected_path` correctly handles these scenarios. This improves reliability of the evolve pipeline's safety checks.
