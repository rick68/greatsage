Title: Remove dead_code warnings from TUI module
Files: src/tui/tui_main.rs
Issue: none

## Description
The TUI code contains several `#[allow(dead_code)]` attributes that suppress warnings for unused code, indicating possible dead code or incomplete implementations. Clean these up to improve code quality and satisfy `cargo clippy`.

### Steps
1. Search for `#[allow(dead_code)]` in `src/tui/tui_main.rs`.
2. For each suppressed item, determine if it is truly unused:
   - If the function/field is used elsewhere, remove the attribute.
   - If unused and not needed, delete the code.
   - If intended for future use, implement a minimal stub or expose it via tests.
3. Run `cargo clippy` to ensure no dead_code warnings remain.
4. Add a test (if appropriate) to cover any newly exposed functionality.
5. Update documentation/comments if any removed items were referenced.

## Acceptance Criteria
- `cargo build` succeeds.
- `cargo test` passes.
- `cargo clippy` shows no `dead_code` warnings.
- No functionality regression (existing TUI behavior unchanged).
