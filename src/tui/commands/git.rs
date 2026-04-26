//! Git slash-command handler (`/git stage`, `/git commit -m …`, `/git revert`).
//!
//! All commands delegate to the [`crate::git`] module which wraps libgit2
//! operations.  Each function returns `Vec<Line<'static>>` with ✅/❌ icons
//! to give instant visual feedback.

use ratatui::{style::Stylize, text::Line};

/// Extracts the commit message string from a `-m` argument.
///
/// Accepts both adjacent (`-m"msg"`) and spaced (`-m msg`) forms, stripping
/// any surrounding single or double quotes.
///
/// Returns an empty string if no `-m` flag is found.
pub fn parse_commit_message(arg: impl AsRef<str>) -> String {
    let arg = arg.as_ref().trim();
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

/// Dispatches `/git <subcommand>` to the correct git operation.
///
/// Supported subcommands:
///
/// | Subcommand | Example | Effect |
/// |------------|---------|--------|
/// | `stage`    | `/git stage` | Stage all changes (`git add -A`) |
/// | `commit`   | `/git commit -m "msg"` | Commit staged changes |
/// | `revert`   | `/git revert` | Revert the last commit |
///
/// Typing `/git` alone (no subcommand) shows the help text.
pub fn handle_git_subcmd(cmd: impl AsRef<str>) -> Vec<Line<'static>> {
    let rest = cmd.as_ref().trim_start_matches("/git").trim();
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
