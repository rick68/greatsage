//! REPL tool execution status lines (`▶` summary + inline ✓/✗).

use crate::utils::truncate;

/// Keep each `▶` status line on one terminal row so parallel ✓ cursor moves stay aligned.
const MAX_TOOL_PATH_CHARS: usize = 60;

/// One-line tool status label shown after `▶` (yoyo-style).
pub(crate) fn format_tool_execution_summary(tool_name: &str, args: &serde_json::Value) -> String {
    match tool_name {
        "bash" => {
            let cmd = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("...");
            format!("$ {}", truncate(cmd, 72))
        }
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("read {}", truncate(path, MAX_TOOL_PATH_CHARS))
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("write {}", truncate(path, MAX_TOOL_PATH_CHARS))
        }
        "edit_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("edit {}", truncate(path, MAX_TOOL_PATH_CHARS))
        }
        "list_files" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("ls {}", truncate(path, MAX_TOOL_PATH_CHARS))
        }
        "search" => {
            let pat = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            format!("search '{}'", truncate(pat, 60))
        }
        _ => tool_name.to_string(),
    }
}

/// Prefix for `▶`. Always starts on a new line below any block-gap separator.
pub(crate) fn format_tool_start_line(summary: &str) -> String {
    format!("\n{}", format_tool_inline_line(summary))
}

/// `▶` status line without a leading newline (for in-place redraw).
pub(crate) fn format_tool_inline_line(summary: &str) -> String {
    format!("  ▶ {summary}")
}

/// Clear the current terminal row before redrawing a prior parallel tool line.
pub(crate) fn tool_line_clear_prefix() -> &'static str {
    "\r\x1b[K"
}

/// First text chunk after a thinking divider: drop provider-leading `\n` so only the
/// single block-gap blank line remains before response text.
pub(crate) fn first_text_after_thinking_block<'a>(
    delta: &'a str,
    pending: &mut bool,
) -> Option<&'a str> {
    if !*pending {
        return Some(delta);
    }
    let trimmed = delta.trim_start_matches('\n');
    if trimmed.is_empty() {
        return None;
    }
    *pending = false;
    Some(trimmed)
}

/// One row in the active parallel tool batch shown in the REPL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolDisplayLine {
    pub summary: String,
    /// `None` = still running; `Some(false)` = ✓; `Some(true)` = ✗
    pub finished: Option<bool>,
}

/// Count tool rows still awaiting ✓/✗.
pub(crate) fn tool_batch_pending_count(lines: &[ToolDisplayLine]) -> usize {
    lines.iter().filter(|line| line.finished.is_none()).count()
}

/// True when all tool lines have received ✓/✗ and output may need a trailing newline.
pub(crate) fn tool_batch_complete(pending_tool_count: usize) -> bool {
    pending_tool_count == 0
}

/// Move cursor from the end of the last batch row back to the first before a full redraw.
pub(crate) fn tool_batch_redraw_cursor_up(line_count: usize) -> String {
    let up = line_count.saturating_sub(1);
    if up == 0 {
        String::new()
    } else {
        format!("\x1b[{up}A")
    }
}
