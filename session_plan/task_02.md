Title: Enforce protected‑file guard in evolve pipeline
Files: src/evolve.rs
Issue: none

Description:
Add runtime enforcement to the evolve pipeline that prevents modifications to protected paths (e.g., `.github/workflows/`, `IDENTITY.md`, `scripts/`, `skills/`). When a task attempts to write to a protected file, the pipeline should:
1. Detect the attempted change via the file‑write utilities.
2. Compare the target path against `is_protected_path`.
3. Abort the task execution, revert any partial changes, and record a clear error message.
4. Ensure the evolve process continues with remaining tasks without crashing.

Implementation steps:
- Locate the function in `src/evolve.rs` that applies task file changes (e.g., `apply_task_changes`).
- Before each file write, call `is_protected_path(&path)`.
- If true, stop processing the current task, log the violation, and return an error that higher‑level logic can handle to skip the task.
- Add a unit test in `tests/` verifying that a task attempting to modify `scripts/evolve.sh` is rejected and does not alter the filesystem.
- Update documentation (e.g., `README.md` or `YOYO.md`) to describe the protected‑file enforcement.

Goal: Strengthen safety guarantees of self‑modifying evolution, addressing the gap identified in the assessment.
