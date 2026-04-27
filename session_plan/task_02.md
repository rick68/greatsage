Title: Enforce protected‑path guardrails during evolution
Files: src/evolve.rs, src/config.rs
Issue: none

Add runtime verification that evolution‑related file modifications never affect protected paths (e.g., `.github/workflows/`, `IDENTITY.md`, `scripts/`, `skills/`). Implement a helper function `is_protected_path(path: &Path) -> bool` in `src/config.rs` and integrate it into `src/evolve.rs` wherever file writes occur (e.g., task creation, journal updates). If a protected path is targeted, abort the operation with a clear error message and log the attempt. This guards against accidental self‑modification of critical files.

Update any related error handling to propagate the guardrail error up to the evolve command exit status.
