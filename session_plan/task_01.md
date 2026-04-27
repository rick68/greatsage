Title: Implement protected‑file enforcement in evolve pipeline
Files: src/evolve.rs
Issue: none

**Description**
Add a function `is_protected_path(path: &str) -> bool` that returns true for paths that must not be modified by the evolution process. The list should include:
- `.github/workflows/`
- `IDENTITY.md`
- `PERSONALITY.md`
- `scripts/`
- `skills/`
- any additional paths currently guarded in `scripts/evolve.sh` (if any).
Update the evolve subcommand stub to call this check before applying any file modifications and abort with an error message if a protected path is targeted.

**Tests**
Create unit tests in `src/evolve.rs` (or a new `src/evolve_test.rs`) that verify:
- Paths matching the above patterns return true.
- Regular source files like `src/main.rs` return false.
- The function correctly handles trailing slashes and nested files (e.g., `scripts/evolve.sh`).

**Documentation**
Update `README.md` section “Evolution Pipeline – Safety” to mention the protected‑file guard and list the protected directories/files.

**Goal**
Provide a safety guard similar to the one in `scripts/evolve.sh`, preventing accidental modification of critical repository files.
