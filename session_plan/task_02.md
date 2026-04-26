Title: Add assessment‑driven task generation in evolve pipeline
Files: src/evolve.rs
Issue: none

Update `planning_phase` to use `assessment_phase` output for generating task titles via `planning_phase_with_assessment`. Ensure protected‑path checks are applied. Modify `run_evolve_with` to call the new flow. This implements a core capability gap: meaningful task planning instead of generic placeholders.
