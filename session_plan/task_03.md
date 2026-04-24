Title: Add unit tests for REPL error handling
Files: src/tui/tui_main.rs, src/tui/mod.rs
Issue: none

## Description
We introduced systematic error handling for REPL commands in task 01. Now we need a test suite to verify that:
1. A failing `git stage` command results in an error line (`❌`) in the REPL output.
2. The REPL state (input cleared, history updated, scroll adjusted) remains consistent after an error.

**Implementation steps**:
- In `src/tui/mod.rs` (or create a `tests` module within `tui`), write a test that mocks `git::stage_all` to return an error (`git2::Error::from_str("mock failure")`).
- Use the `handle_input_area_input` system directly by constructing a `TuiMain` instance, a mock `MessageReader` with a simulated `Enter` key event containing `git stage`, and a mock `CodingAgentPromptChannel`.
- Verify that after processing, `tui_main.output` contains a `Line` with the red error message, and `tui_main.input` is empty.

**Tests**:
- Add a test function `test_git_stage_error_handling` that asserts the above behavior.
- Ensure the test compiles and runs with `cargo test`.

**Documentation**:
No documentation changes needed.

**Scope**: Adds a new test file under `src/tui/tests.rs` (or within `mod.rs` under `#[cfg(test)]`).