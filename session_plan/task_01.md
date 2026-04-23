Title: Add basic Git integration wrapper
Files: src/git.rs, src/agents/mod.rs, src/agents/mod_test.rs
Issue: none

Implement a lightweight Git helper module to enable self‑modification awareness. Create `src/git.rs` exposing functions:
- `fn stage_all() -> Result<(), String>` – runs `git add .` and returns an error string on failure.
- `fn commit(message: &str) -> Result<(), String>` – runs `git commit -m "..."`.
- `fn revert_last() -> Result<(), String>` – runs `git revert HEAD` (or `git reset --hard HEAD~1`).

Add this module to the agents system: in `src/agents/mod.rs` add a public struct `GitAgent` with methods that call the above functions and expose them to the LLM via the tool interface (e.g., tool name `git_stage`, `git_commit`, `git_revert`).

Write unit tests in `src/agents/mod_test.rs` to verify that when the repository is clean, `stage_all` succeeds, and that committing with an empty repo returns an appropriate error (mock by creating a temporary directory). Use the `tempfile` crate for isolation.

Update `Cargo.toml` if needed to add `tempfile` as a dev‑dependency.

Documentation: add a brief note in `README.md` under "Git integration" describing the new commands.
