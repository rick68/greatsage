Title: Add basic git status tool
Files: src/agents/mod.rs, src/agents/coding.rs, src/agents/tests.rs
Issue: none

Introduce a new tool `git_status` that runs `git status --porcelain` in the current repository and returns a concise list of changed files. Add its definition to the tool enum in `src/agents/mod.rs`. In `coding.rs`, implement handling for this tool by invoking the existing `bash` execution helper, capturing stdout, and returning it as the tool result. Provide clear error messages if the command fails (e.g., not a git repository). Add a unit test in `src/agents/tests.rs` that creates a temporary directory, initializes a git repo, creates a file, and asserts that `git_status` returns the expected entry. Update README to document the new git tool.
