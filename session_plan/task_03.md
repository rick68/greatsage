Title: Add unit tests for truncate utility
Files: src/agents/coding.rs
Issue: none

Add a `#[cfg(test)]` module at the end of `src/agents/coding.rs` containing tests for the `truncate` function:
- Test that truncating a string shorter than max returns the original string.
- Test that truncating a string exactly at max returns the original string.
- Test that truncating a longer string returns the substring up to the max character boundary (not splitting a Unicode grapheme). Use a sample string with multibyte characters (e.g., "🦀Rust") to verify correct behavior.
- Ensure the tests compile and `cargo test` passes.

These tests provide initial coverage for a core utility and establish a testing baseline for future work.
