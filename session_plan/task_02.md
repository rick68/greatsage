Title: Document REPL commands in README
Files: README.md
Issue: none

## Description
The README currently mentions the REPL but does not list the available commands (e.g., `/clear`, `/exit`, git shortcuts). Add a section under "REPL usage" documenting these commands with examples, and note that errors are now reported in the output.

**Implementation steps**:
1. Locate the REPL section in `README.md` (or create one if missing).
2. Add a bullet list of supported commands:
   - `/clear` – clears the REPL output.
   - `/exit` or `/quit` – exits the application.
   - `git stage` – stages all changes.
   - `git commit -m "msg"` – commits staged changes.
   - `git revert` – reverts the last commit.
3. Mention that errors from these commands are now displayed in red with a ❌ icon.
4. Ensure markdown formatting is clean.

**Tests**:
No code changes, so no tests needed.

**Documentation**:
Updates the README to improve user onboarding.
