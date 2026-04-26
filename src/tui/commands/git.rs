use ratatui::{
    style::Stylize,
    text::Line,
};

/// Parses the content from a Git commit message argument.
pub fn parse_commit_message(arg: &str) -> String {
    let arg = arg.trim();
    if let Some(after_m) = arg.strip_prefix("-m") {
        after_m
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string()
    } else if let Some(idx) = arg.find("-m ") {
        arg[idx + 3..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string()
    } else {
        String::new()
    }
}

/// Handles Git subcommands.
pub fn handle_git_subcmd(cmd: &str) -> Vec<Line<'static>> {
    let rest = cmd.trim_start_matches("/git").trim();
    let (subcmd, arg) = rest
        .split_once(' ')
        .map(|(s, a)| (s.trim(), a.trim()))
        .unwrap_or((rest, ""));

    match subcmd {
        "stage" => match crate::git::stage_all() {
            Ok(_) => vec![Line::from("✅ Staged all changes").green()],
            Err(e) => vec![Line::from(format!("❌ /git stage failed: {e}")).red()],
        },
        "commit" => {
            let msg = parse_commit_message(arg);
            if msg.is_empty() {
                vec![Line::from("❌ /git commit missing -m message").red()]
            } else {
                match crate::git::commit(&msg) {
                    Ok(_) => vec![Line::from(format!("✅ Commit: {msg}")).green()],
                    Err(e) => vec![Line::from(format!("❌ /git commit failed: {e}")).red()],
                }
            }
        }
        "revert" => match crate::git::revert_last() {
            Ok(_) => vec![Line::from("✅ Reverted last commit").green()],
            Err(e) => vec![Line::from(format!("❌ /git revert failed: {e}")).red()],
        },
        "" => super::builtin::help_lines(),
        _ => vec![Line::from(format!("❌ Unknown /git subcommand: {subcmd}")).red()],
    }
}
