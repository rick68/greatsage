Title: Implement Phase A2 Planning in evolve pipeline
Files: src/evolve.rs, src/cli.rs
Issue: none

## Description
Add the planning phase (Phase A2) that follows the assessment.
- Create a function `run_planning(assessment: &AssessmentResult) -> Vec<Task>` in `src/evolve.rs`.
- The function parses the assessment report, selects up to three high‑priority items (e.g., missing error‑handling flag, protected‑path guard, or other gaps) and creates task markdown files `task_*.md` in the `session_plan/` directory.
- Each generated task should include a title, affected files, and a brief description, matching the format used for session planning.
- Integrate this function into the evolve flow so that after `run_assessment` the planning step runs automatically.
- Add a simple prioritization: prioritize tasks that address capability gaps listed in the assessment (e.g., “missing error‑handling flag”).
- Write unit tests verifying that given a mock `AssessmentResult` with several items, `run_planning` creates at most three task files and includes expected titles.
- Update CLI help to reflect that `--evolve` now includes planning after assessment.

### Acceptance Criteria
- `cargo build && cargo test` passes.
- Invoking `greatsage --evolve` executes assessment then planning, creating up to three `task_*.md` files in `session_plan/`.
- Unit tests confirm task creation limits and content.
- Documentation updated in `README.md` under "Evolution Pipeline" to mention Phase A2.
