use ratatui::{
    style::Stylize,
    text::Line,
};

/// Generates help text lines.
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
