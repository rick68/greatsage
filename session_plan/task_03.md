Title: Show current Git branch in REPL prompt
Files: src/main.rs, src/agents/coding.rs
Issue: none

Implement a small feature that detects if the current working directory is inside a Git repository and, if so, obtains the current branch name. Display this branch name as part of the REPL prompt (e.g., "greatsage (main)> ") to give the user context.

Steps:
1. In `src/main.rs` (or a new small helper module) add a function `fn current_git_branch() -> Option<String>` that runs `git rev-parse --abbrev-ref HEAD` via `std::process::Command` and returns `Some(branch)` on success, `None` otherwise.
2. Pass the detected branch (if any) to the coding agent, perhaps by inserting it into the prompt string when initializing the REPL in `src/agents/coding.rs`.
3. Modify the REPL prompt generation in `src/agents/coding.rs` to include the branch name when present.
4. Write a unit test for the helper function using a temporary directory with a git repo (use `tempfile` crate) to ensure it returns the correct branch, and a test for a non‑git directory returning `None`.
5. Ensure the change does not affect existing functionality; all existing tests must still pass.
6. Update documentation (`GREATSAGE.md` or README) to mention the new branch‑aware prompt.

This task touches at most two source files and adds a small utility plus a prompt update, fitting within a 20‑minute implementation window.