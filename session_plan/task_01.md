Title: Add Git tool integration to REPL
Files: src/agents/coding.rs, src/agents/mod.rs, src/git.rs
Issue: none

Implement a new tool named `git` accessible to the LLM via the REPL. Provide sub‑commands:
- `status` – display repository status (list modified, added, deleted files) using `git2`.
- `add .` – stage all changes (reuse `git::stage_all`).
- `commit -m "msg"` – create a commit with the supplied message (reuse `git::commit`).
- `diff` – show a diff of staged changes (use `git2` diff APIs).
- `revert` – undo the last commit (reuse `git::revert_last`).
Add a dispatcher in `agents::coding` that parses the first argument as the sub‑command and calls the corresponding helper in `git.rs`. Register the tool in `agents::mod` alongside existing tools. Keep permissions simple: treat the tool as read‑only except for `add` and `commit`, which are allowed because the REPL already trusts the user.
Update README to mention the new git capabilities.
Ensure the code compiles and all existing tests pass.
