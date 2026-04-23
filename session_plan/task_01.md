Title: Expose Git actions in REPL commands
Files: src/agents/coding.rs, src/git.rs, docs/src/usage.md
Issue: none

Implement REPL commands for common Git operations to match Claude Code capabilities.
- Add command parsing for "git stage", "git commit -m <msg>", and "git revert" within `agents::coding::handle_user_input` (or similar entry point).
- Use existing functions in `git.rs` (`stage_all`, `commit`, `revert_last`) to perform actions.
- Provide user feedback in the REPL output (success or error messages).
- Update documentation (docs/src/usage.md) to describe the new commands.
- Ensure the changes compile and tests (if any) pass.
