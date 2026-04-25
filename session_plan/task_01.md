Title: Improve protected path detection in evolve.rs
Files: src/evolve.rs, src/tests/evolve_protection.rs, README.md
Issue: none

Update the `is_protected_path` function to use path component matching instead of substring matching, preventing false positives (e.g., "scripts_backup" should not be protected). Add unit tests covering protected and non‑protected cases, and update the README to describe the new logic.
