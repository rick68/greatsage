Title: Add clear output command
Files: src/tui/tui_main.rs
Issue: none

## Description
Implement a way for the user to clear the REPL output buffer during a session.

- Add a method `fn clear_output(&mut self)` to `TuiMain` that empties `self.output` and resets scrolling state.
- Extend the input handling in `handle_input_area_input` to recognize the command `"/clear"` (entered at the prompt) and invoke the new method.
- Ensure the UI updates by scrolling to bottom after clearing.
- Update any related UI state if necessary.
- Add a unit test verifying that `clear_output` empties the output vector and resets scroll.
- No changes to existing functionality beyond handling the new command.

## Documentation
Update the README (or relevant docs) to mention the new `/clear` command.
