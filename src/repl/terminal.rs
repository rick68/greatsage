//! REPL stdout formatting and input-line redraw helpers.
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.

use {
    super::completion::{
        apply_replacement, format_candidate_columns, inline_hint, str_display_width,
    },
    crate::{agents::AgentConfig, cli::Cli, stdout::StdoutMessage},
    bevy::ecs::message::MessageWriter,
    bevy_ratatui::crossterm,
    colored::Colorize,
};

fn terminal_columns() -> usize {
    crossterm::terminal::size()
        .map(|(width, _)| width as usize)
        .unwrap_or(80)
}

fn is_model_info_title(line: &str) -> bool {
    model_info_title_name(line).is_some()
}

fn model_info_title_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix("── ")
        .and_then(|rest| rest.strip_suffix(" ──"))
}

fn is_model_info_output(output: &[String]) -> bool {
    output.iter().any(|line| is_model_info_title(line))
}

fn is_tokens_output(output: &[String]) -> bool {
    output.first().is_some_and(|line| line == "Active context:")
}

fn is_model_info_separator(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && trimmed.chars().all(|c| c == '─')
}

fn is_tokens_context_warning(line: &str) -> bool {
    (line.contains('⚠') && line.contains("Context is getting full"))
        || line.contains("Context nearly full")
}

fn is_tokens_low_remaining(line: &str) -> bool {
    if !line.contains("remaining") || line.contains("Context nearly full") {
        return false;
    }
    let Some(rest) = line.trim().strip_prefix('~') else {
        return false;
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    let Ok(count) = digits.parse::<usize>() else {
        return false;
    };
    count <= 3
}

const TOKENS_SESSION_TOTALS_HEADER: &str = "Session totals (all API calls):";

fn style_tokens_line(line: &str, in_session_totals: bool) -> String {
    if in_session_totals || line == TOKENS_SESSION_TOTALS_HEADER {
        return format!("  {line}");
    }
    if line.contains("Context nearly full") {
        return format!("  {line}").red().to_string();
    }
    if is_tokens_context_warning(line) || is_tokens_low_remaining(line) {
        return format!("  {line}").yellow().to_string();
    }
    style_repl_output_line(line)
}

fn style_model_info_value(value: &str) -> String {
    const CHECK: char = '✓';
    if value.trim() == "unknown" {
        return value.yellow().to_string();
    }
    if let Some(pos) = value.find(CHECK) {
        let before = &value[..pos];
        let after = &value[pos + CHECK.len_utf8()..];
        return format!("{before}{}{after}", CHECK.to_string().green());
    }
    value.to_string()
}

fn style_model_info_line(line: &str) -> String {
    const INDENT: &str = "  ";
    if line.is_empty() {
        return String::from(INDENT);
    }
    if is_model_info_separator(line) {
        return format!("{INDENT}{}", line.trim().dimmed());
    }
    if model_info_title_name(line).is_some() {
        return format!("{INDENT}{}", line.trim().bold());
    }
    if let Some((label, value)) = line.split_once(':') {
        return format!(
            "{INDENT}{}{}",
            format!("{label}:").dimmed(),
            style_model_info_value(value)
        );
    }
    format!("{INDENT}{}", line.dimmed())
}

fn style_repl_output_line(line: &str) -> String {
    if line.starts_with("unknown provider:") || line.starts_with("No models match") {
        format!("  {line}").yellow().to_string()
    } else {
        format!("  {line}").dimmed().to_string()
    }
}

pub(super) fn write_repl_response(stdout: &mut MessageWriter<StdoutMessage>, text: &str) {
    for line in text.lines() {
        let styled = style_repl_output_line(line);
        stdout.write(StdoutMessage::from(format!("{styled}\n")));
    }
}

pub(super) fn write_unknown_slash_feedback(
    stdout: &mut MessageWriter<StdoutMessage>,
    typed: &str,
    suggestion: Option<&str>,
) {
    stdout.write(StdoutMessage::from(
        format!("  unknown command: {typed}\n").red(),
    ));
    if let Some(suggestion) = suggestion {
        stdout.write(StdoutMessage::from(
            format!("  did you mean {suggestion}?\n").yellow(),
        ));
    }
    stdout.write(StdoutMessage::from(
        String::from("  type /help for available commands\n").dimmed(),
    ));
}

pub(super) fn write_repl_detail_line(
    stdout: &mut MessageWriter<StdoutMessage>,
    line: &str,
    trailing_newline: bool,
) {
    let styled = line.dimmed().to_string();
    if trailing_newline {
        stdout.write(StdoutMessage::from(format!("{styled}\n")));
    } else {
        stdout.write(StdoutMessage::from(styled));
    }
}

pub(super) fn write_repl_handled_output(
    stdout: &mut MessageWriter<StdoutMessage>,
    output: &[String],
    detail: &[String],
) {
    let model_info = is_model_info_output(output);
    let tokens_output = is_tokens_output(output);
    let mut in_session_totals = false;
    for line in output {
        if tokens_output && line == TOKENS_SESSION_TOTALS_HEADER {
            in_session_totals = true;
        }
        let styled = if model_info {
            style_model_info_line(line)
        } else if tokens_output {
            style_tokens_line(line, in_session_totals)
        } else {
            style_repl_output_line(line)
        };
        stdout.write(StdoutMessage::from(format!("{styled}\n")));
    }
    if !detail.is_empty() {
        if !output.is_empty() {
            stdout.write(StdoutMessage::newline());
        }
        for line in detail {
            () = write_repl_detail_line(stdout, line, true);
        }
    }
}

pub(super) fn write_repl_response_lines(
    stdout: &mut MessageWriter<StdoutMessage>,
    lines: &[String],
) {
    () = write_repl_handled_output(stdout, lines, &[]);
}

pub(super) fn write_repl_candidate_list(
    stdout: &mut MessageWriter<StdoutMessage>,
    candidates: &[String],
) {
    stdout.write(StdoutMessage::newline());
    for line in format_candidate_columns(candidates, terminal_columns()) {
        stdout.write(StdoutMessage::from(format!("{line}\n")));
    }
}

pub(super) fn sync_inline_hint(
    stdout: &mut MessageWriter<StdoutMessage>,
    content: &str,
    cursor: usize,
    agent_config: &AgentConfig,
    hint_width: &mut usize,
) {
    if *hint_width > 0 {
        stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
        *hint_width = 0;
    }
    if cursor == content.chars().count()
        && let Some(hint) = inline_hint(content, cursor, agent_config)
    {
        *hint_width = str_display_width(&hint);
        stdout.write(StdoutMessage::from(hint.dimmed().to_string()));
        for _ in 0..*hint_width {
            stdout.write(StdoutMessage::move_cursor_left());
        }
    }
}

pub(super) fn write_repl_candidate_list_truncated(
    stdout: &mut MessageWriter<StdoutMessage>,
    candidates: &[String],
    limit: usize,
) {
    let (shown, hidden) = if candidates.len() > limit {
        (&candidates[..limit], candidates.len() - limit)
    } else {
        (candidates, 0)
    };
    () = write_repl_candidate_list(stdout, shown);
    if hidden > 0 {
        stdout.write(StdoutMessage::from(format!(
            "  ... and {hidden} more — type more characters to narrow\n"
        )));
    }
}

pub(super) fn write_quit_farewell_if_enabled(stdout: &mut MessageWriter<StdoutMessage>, cli: &Cli) {
    if !cli.print_system_prompt && !cli.no_hints {
        stdout.write(StdoutMessage::newline());
        stdout.write(StdoutMessage::newline());
        () = write_repl_response(stdout, "bye 👋");
    }
}

pub(super) fn apply_token_replacement(
    content: &mut String,
    cursor: &mut usize,
    stdout: &mut MessageWriter<StdoutMessage>,
    start_char: usize,
    end_char: usize,
    replacement: &str,
    agent_config: &AgentConfig,
    hint_width: &mut usize,
) {
    if *hint_width > 0 {
        stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
        *hint_width = 0;
    }
    let old_token: String = content
        .chars()
        .skip(start_char)
        .take(end_char - start_char)
        .collect();
    let suffix: String = content.chars().skip(end_char).collect();
    let old_token_width = str_display_width(&old_token);

    *content = apply_replacement(content, start_char, end_char, replacement);
    *cursor = start_char + replacement.chars().count();

    for _ in 0..old_token_width {
        stdout.write(StdoutMessage::move_cursor_left());
    }
    stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
    let tail_byte = content
        .char_indices()
        .nth(start_char)
        .map(|(i, _)| i)
        .unwrap_or(content.len());
    stdout.write(StdoutMessage::from(&content[tail_byte..]));
    if !suffix.is_empty() {
        let suffix_width = str_display_width(&suffix);
        for _ in 0..suffix_width {
            stdout.write(StdoutMessage::move_cursor_left());
        }
    }
    () = sync_inline_hint(stdout, content, *cursor, agent_config, hint_width);
}

pub(super) fn redraw_input_line(
    stdout: &mut MessageWriter<StdoutMessage>,
    content: &str,
    cursor: usize,
    agent_config: &AgentConfig,
    hint_width: &mut usize,
) {
    stdout.write(StdoutMessage::from(super::prompt_symbol()));
    stdout.write(StdoutMessage::from(content));
    () = sync_inline_hint(stdout, content, cursor, agent_config, hint_width);
}

/// Erase type-ahead characters echoed inline at the output cursor (backspace over display width).
pub(super) fn erase_ahead_echo(stdout: &mut MessageWriter<StdoutMessage>, display_width: usize) {
    for _ in 0..display_width {
        stdout.write(StdoutMessage::from("\x08 \x08"));
    }
}

/// Replace the current prompt line without a leading newline (history ↑↓, idle prompt refresh).
pub(super) fn replace_input_line_in_place(
    stdout: &mut MessageWriter<StdoutMessage>,
    content: &str,
    cursor: usize,
    agent_config: &AgentConfig,
    hint_width: &mut usize,
) {
    if *hint_width > 0 {
        stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
        *hint_width = 0;
    }
    let prompt = super::prompt_symbol_inline();
    stdout.write(StdoutMessage::from("\r"));
    stdout.write(StdoutMessage::from(format!("{prompt}{content}")));
    stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
    () = sync_inline_hint(stdout, content, cursor, agent_config, hint_width);
}
