Title: Update README with new execute_tasks feature
Files: src/evolve.rs, README.md
Issue: none

Add a short section to the README describing the new `execute_tasks` function added to the evolve pipeline. Explain that after the planning phase, `run_evolve_with` now runs each task, logging execution to `evolve.log`. Keep the change minimal, inserting the description under a new "Evolution Pipeline" heading.
