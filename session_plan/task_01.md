Title: Implement task execution phase in evolve pipeline
Files: src/evolve.rs
Issue: none

Add a new function `execute_tasks` that reads the markdown files in `session_plan/`, parses the `Title` line, and prints a message indicating execution of each task. Update `run_evolve_with` to call this function after the planning phase. Ensure the implementation respects the protected‑path checks and only processes files ending with `.md`. Write a unit test `test_execute_tasks` verifying that for a temporary `session_plan` containing a task file, the function runs without error and creates a log file `evolve.log` with the task titles.
