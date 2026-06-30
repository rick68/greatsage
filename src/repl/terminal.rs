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
    let trimmed = line.trim();
    trimmed.starts_with("── ") && trimmed.ends_with(" ──")
}

fn is_model_info_output(output: &[String]) -> bool {
    output.iter().any(|line| is_model_info_title(line))
}

fn style_model_info_value(value: &str) -> String {
    const CHECK: char = '✓';
    if let Some(pos) = value.find(CHECK) {
        let before = &value[..pos];
        let after = &value[pos + CHECK.len_utf8()..];
        return format!(
            "{}{}{}",
            before.white(),
            CHECK.to_string().green(),
            after.white()
        );
    }
    value.white().to_string()
}

fn style_model_info_line(line: &str) -> String {
    const INDENT: &str = "  ";
    if line.is_empty() {
        return String::from(INDENT);
    }
    if is_model_info_title(line) {
        return format!("{INDENT}{}", line.bright_white());
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
        format!("  type /help for available commands\n").dimmed(),
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
    for line in output {
        let styled = if model_info {
            style_model_info_line(line)
        } else {
            style_repl_output_line(line)
        };
        stdout.write(StdoutMessage::from(format!("{styled}\n")));
    }
    if !detail.is_empty() {
        if !output.is_empty() {
            stdout.write(StdoutMessage::newline());
        }
        let last = detail.len() - 1;
        for (i, line) in detail.iter().enumerate() {
            () = write_repl_detail_line(stdout, line, i < last);
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
