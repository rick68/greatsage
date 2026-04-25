Title: Add unit tests for truncate utility
Files: src/agents/coding.rs, src/tests/truncate.rs
Issue: none

Add comprehensive unit tests for the `truncate` function used to shorten strings in tool output summaries. Tests should cover:
- Truncating a plain ASCII string shorter than the limit (returns original).
- Truncating a string exactly at the limit (returns original).
- Truncating a string longer than the limit (returns shortened version without breaking Unicode characters).
- Handling Unicode characters that are multi-byte (ensuring no panic and correct cut).

Create a new test file at `src/tests/truncate.rs` containing these cases.

Update `Cargo.toml` if needed (no extra deps).
