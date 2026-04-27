Title: Add checkpoint‑restart support to evolve pipeline
Files: src/evolve.rs
Issue: none

Create a simple checkpoint system for the evolve pipeline. Before executing tasks, record the current Git HEAD hash to a temporary file (e.g., `.greatsage/evolve_checkpoint`). If the pipeline is interrupted (e.g., panic or early exit), on the next run detect the checkpoint file and resume from the last successful phase, allowing up to two attempts per phase. Implement functions `create_checkpoint`, `load_checkpoint`, and integrate them in `run_evolve_with` to ensure resilience without exceeding the 2‑attempt budget.

