//! Slash-command dispatcher and sub-command implementations.
//!
//! When the user submits a line starting with `/`, the action system calls
//! [`handle_slash_command`] which routes to the appropriate handler module.
//!
//! # Adding a new command
//!
//! 1. Create a new sub-module (e.g. `pub mod mycommand;`).
//! 2. Implement a function that returns `Vec<Line<'static>>`.
//! 3. Add a match arm in [`handle_slash_command`] for your base token.

pub mod builtin;
pub mod git;

use ratatui::text::Line;

/// Entry point for all slash commands typed in the input box.
///
/// Extracts the base token (first whitespace-delimited word) and routes to:
/// * `/help` → [`builtin::help_lines`]
/// * `/git …` → [`git::handle_git_subcmd`]
/// * anything else → an "unknown command" error line
///
/// Returns a `Vec<Line<'static>>` that the caller appends to the output panel.
pub fn handle_slash_command(cmd: impl AsRef<str>) -> Vec<Line<'static>> {
    let base = cmd
        .as_ref()
        .split_whitespace()
        .next()
        .unwrap_or(cmd.as_ref());
    match base {
        "/help" => builtin::help_lines(),
        "/git" => git::handle_git_subcmd(cmd.as_ref()),
        _ => vec![Line::from(format!("❌ Unknown command: {}", cmd.as_ref()))],
    }
}
