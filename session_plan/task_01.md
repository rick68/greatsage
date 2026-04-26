Title: Replace placeholder planning with assessment‑driven task generation
Files: src/evolve.rs
Issue: none

Replace the current `planning_phase` that always creates three generic placeholder tasks with a flow that uses the assessment output to generate meaningful task titles. Update `run_evolve_with` to call `assessment_phase` then `planning_phase_with_assessment`. Ensure the new function respects protected‑path checks. This brings the evolve pipeline closer to the behavior of `scripts/evolve.sh`.
