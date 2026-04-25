Title: Add sponsor‑gating logic to --evolve command
Files: src/evolve.rs, src/main.rs, sponsors/sponsor_info.json
Issue: none

Implement runtime gating for the evolve operation based on sponsor status:
1. Load sponsor info from `sponsors/sponsor_info.json` (contains last_run timestamp and allowed interval).
2. In `src/main.rs` before invoking `evolve::run_evolve()`, check the gate:
   - If a sponsor flag (e.g., args.sponsor) is present, bypass the gate.
   - Otherwise, ensure at least 8 hours have passed since last run; if not, exit with a clear message.
3. On successful start of evolve, update the `last_run` timestamp atomically (write to a temporary file then rename) to prevent race conditions.
4. Add a new CLI option `--sponsor` (bool) to indicate a sponsor‑provided run.
5. Update `src/evolve.rs` with a helper `fn check_sponsor_gate() -> Result<(), String>` that returns an error when the gate blocks execution.
6. Add unit tests for the gate logic covering: allowed run, blocked run, sponsor override, atomic update.
7. Ensure `cargo build` and `cargo test` pass.

Documentation: add a short note in README under the `--evolve` description about the 8‑hour gate and sponsor override.
