//! Built-in slash commands that don't interact with external tools.
//!
//! Currently only `/help` lives here.  As new built-in commands are added
//! (e.g. `/config`, `/version`) they should go in this module.

use ratatui::{style::Stylize, text::Line};

/// Returns formatted help text listing all available commands and shortcuts.
///
/// Called by [`super::handle_slash_command`] when the user types `/help`.
pub fn help_lines() -> Vec<Line<'static>> {
    vec![
        Line::raw(""),
        Line::from("Commands (in REPL):".bold()),
        Line::raw(""),
        Line::from("  Session:".cyan().bold()),
        Line::raw("    /help              Show this help"),
        Line::raw("    /clear             Clear output"),
        Line::raw("    /quit, /exit       Exit greatsage"),
        Line::raw(""),
        Line::from("  Git:".cyan().bold()),
        Line::raw("    /git stage         Stage all changes"),
        Line::raw("    /git commit -m …   Commit staged changes"),
        Line::raw("    /git revert        Revert last commit"),
        Line::raw(""),
        Line::from("  Keyboard shortcuts:".cyan().bold()),
        Line::raw("    Tab                Switch focus (Input ↔ Output)"),
        Line::raw("    ↑/↓  PgUp/PgDn    Scroll output (in Output area)"),
        Line::raw("    t                  Toggle last thinking block (Output area)"),
        Line::raw("    A                  Expand all thinking blocks (Output area)"),
        Line::raw("    a                  Collapse all thinking blocks (Output area)"),
        Line::raw("    Ctrl+C             Exit"),
        Line::raw(""),
    ]
}
