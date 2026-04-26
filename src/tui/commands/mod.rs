pub mod builtin;
pub mod git;

use ratatui::text::Line;

/// Handles slash commands.
pub fn handle_slash_command(cmd: &str) -> Vec<Line<'static>> {
    let base = cmd.split_whitespace().next().unwrap_or(cmd);
    match base {
        "/help" => builtin::help_lines(),
        "/git" => git::handle_git_subcmd(cmd),
        _ => vec![Line::from(format!("❌ Unknown command: {cmd}"))],
    }
}
