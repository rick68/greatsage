Title: Add systematic error handling to REPL command execution
Files: src/tui/tui_main.rs
Issue: none

## Description
The REPL currently executes git commands and sends user input to the coding agent without robust error handling. On failure (e.g., `git stage` returning an error), the UI may not reflect the problem and could lead to silent failures.

**Implementation steps**:
1. Introduce a helper `fn handle_git_command(tui: &mut TuiMain, cmd: &str) -> Line<'static>` that executes the corresponding git operation (`stage_all`, `commit`, `revert`) and returns a colored `Line` indicating success or error.
2. In `handle_input_area_input`, replace the inline git command handling with a call to this helper, propagating any errors as a line in the output.
3. Wrap the channel send (`channel.sender.send(input)`) in a `match` that logs the error via `eprintln!` and also pushes an error line to the REPL output so the user sees the failure.
4. Ensure that the REPL continues operating after any error – the UI state (history, scroll, input) should be reset as before.

**Tests**:
- Add a unit test for `handle_git_command` that simulates a failing `git stage` (by mocking `git::stage_all` to return an error) and asserts the returned line is red and contains "❌".
- Verify that after handling a failing command the REPL state (`input` cleared, scroll updated) remains consistent.

**Documentation**:
Update the README section describing REPL commands to note that errors are now reported in the output.

**Scope**: Modifies only `src/tui/tui_main.rs` (plus associated test block).