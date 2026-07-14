//! UI-only command helpers (quit, focus); agent slash stays in `repl` dispatch.

/// Returns true when the line is a quit request that should exit the TUI.
pub fn is_ui_quit_line(line: &str) -> bool {
    matches!(
        line.trim(),
        "/quit" | "/exit" | "/q" | "quit" | "exit" | ":q"
    )
}
