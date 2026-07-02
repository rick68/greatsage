//! REPL tool execution status lines (`▶` summary + inline ✓/✗).

use crate::utils::truncate;

/// One-line tool status label shown after `▶` (yoyo-style).
pub(crate) fn format_tool_execution_summary(tool_name: &str, args: &serde_json::Value) -> String {
    match tool_name {
        "bash" => {
            let cmd = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("...");
            format!("$ {}", truncate(cmd, 80))
        }
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("read {path}")
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("write {path}")
        }
        "edit_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            format!("edit {path}")
        }
        "list_files" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("ls {path}")
        }
        "search" => {
            let pat = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            format!("search '{}'", truncate(pat, 60))
        }
        _ => tool_name.to_string(),
    }
}

/// ANSI cursor moves to place an inline checkmark on a prior tool line when ends batch after starts.
pub(crate) fn tool_checkmark_cursor_moves(
    line_idx: usize,
    last_line_idx: usize,
) -> (String, String) {
    let lines_up = last_line_idx.saturating_sub(line_idx);
    if lines_up == 0 {
        return (String::new(), String::new());
    }
    (format!("\x1b[{lines_up}A"), format!("\x1b[{lines_up}B"))
}
