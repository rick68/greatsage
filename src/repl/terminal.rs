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

fn is_init_output(output: &[String]) -> bool {
    output
        .first()
        .is_some_and(|line| line == "Scanning project...")
}

fn is_memory_output(output: &[String]) -> bool {
    output.first().is_some_and(|line| {
        line.starts_with("usage: /remember")
            || line.starts_with("usage: /forget")
            || line.starts_with("No project memories")
            || line.starts_with("Project memories (")
            || line.starts_with("Found ")
            || line.starts_with("✓ Remembered:")
            || line.starts_with("✓ Forgot:")
            || line.starts_with("error:")
            || line.starts_with("No memories matching")
    })
}

/// `/history detail` body: optional leading blank, then `Turn N` / `Total:` lines.
fn is_history_detail_output(output: &[String]) -> bool {
    output.iter().any(|line| {
        let t = line.trim_start();
        t.starts_with("Turn ") || t.starts_with("Total:")
    })
}

/// `/cd` success or bare pwd: path (white); optional yoyo context note (dim).
fn is_cd_output(output: &[String]) -> bool {
    match output.len() {
        1 => {
            // bare `/cd` pwd or single path line — treat absolute paths as cd output
            let line = output[0].as_str();
            !line.is_empty()
                && (line.starts_with('/') || line.starts_with('~') || line.starts_with("✗ "))
        }
        2 => output[1].starts_with("(project context was loaded"),
        _ => false,
    }
}

fn style_cd_line(line: &str) -> String {
    if line.starts_with("✗ ") {
        format!("  {line}").red().to_string()
    } else if line.starts_with("(project context was loaded") {
        format!("  {line}").dimmed().to_string()
    } else {
        // path expression: white (yoyo uncolored / white)
        format!("  {line}").white().to_string()
    }
}

/// `/run` / `!` result: stdout body white; exit summary white on success, red on failure.
/// Must not match `/bg list` rows (those contain `✗ exit` mid-line after `[id]`).
/// Must not match `/bg output` (job body may contain exit-looking lines).
fn is_run_output(output: &[String]) -> bool {
    if is_bg_list_output(output) || is_bg_output_output(output) {
        return false;
    }
    output
        .iter()
        .any(|line| line.starts_with("✓ exit ") || line.starts_with("✗ exit "))
}

/// `/bg output`: every content line is tagged with [`crate::repl::shell_bg::BG_OUTPUT_BODY_PREFIX`].
fn is_bg_output_output(output: &[String]) -> bool {
    use crate::repl::shell_bg::BG_OUTPUT_BODY_PREFIX;
    output
        .iter()
        .any(|line| line.starts_with(BG_OUTPUT_BODY_PREFIX))
}

/// Flush-left white (job capture body; omit header and empty notice included).
fn style_bg_output_line(line: &str) -> String {
    use crate::repl::shell_bg::BG_OUTPUT_BODY_PREFIX;
    let body = line.strip_prefix(BG_OUTPUT_BODY_PREFIX).unwrap_or(line);
    body.white().to_string()
}

/// `/bg list` (and bare `/bg`): header or empty message — flush-left, no REPL 2-space indent.
fn is_bg_list_output(output: &[String]) -> bool {
    output.first().is_some_and(|line| {
        line == "No background jobs"
            || line == "Background Jobs"
            // tolerate accidental leading spaces
            || line.trim() == "No background jobs"
            || line.trim() == "Background Jobs"
    })
}

fn style_bg_list_line(line: &str) -> String {
    // Flush-left (no extra REPL indent). Row table indent `  [id] …` kept as-is.
    // Use explicit `.to_string()` on each Colorize segment so ANSI is baked in
    // before `format!` (colored 3 + multi-segment).
    let trimmed = line.trim();
    if trimmed == "Background Jobs" {
        // bold green (not bright)
        return "Background Jobs".bold().green().to_string();
    }
    if trimmed == "No background jobs" {
        return "No background jobs".dimmed().to_string();
    }

    // Row: `  [id]  {status}  {elapsed}  {cmd}` (fields separated by two spaces).
    let indent = if line.starts_with("  ") { "  " } else { "" };
    let body = line.trim_start();
    let Some(close) = body.find(']') else {
        return line.dimmed().to_string();
    };
    if !body.starts_with('[') {
        return line.dimmed().to_string();
    }
    let id_part = &body[..=close];
    let after = body[close + 1..].trim_start();
    let mut parts = after.splitn(3, "  ");
    let status = parts.next().unwrap_or("");
    let elapsed = parts.next().unwrap_or("");
    let cmd = parts.next().unwrap_or("");

    let status_styled = if status.contains('✓') {
        // ✓ done — green
        status.green().to_string()
    } else if status.contains('✗') {
        // ✗ exit N / ✗ done — red
        status.red().to_string()
    } else if status.contains('●') {
        // ● running — yellow (yoyo)
        status.yellow().to_string()
    } else {
        String::from(status)
    };

    format!(
        "{indent}{}  {}  {}  {}",
        id_part.bright_white().to_string(),
        status_styled,
        elapsed.dimmed().to_string(),
        cmd.white().to_string()
    )
}

/// `/bg run` started: `⚡ Background job [id] started: <cmd>` — flush-left.
/// After ⚡ and before `:`: green; `[…]` bright green; command after `:` dim.
fn is_bg_started_line(line: &str) -> bool {
    line.starts_with("⚡ Background job")
}

fn style_bg_started_line(line: &str) -> String {
    let Some(after_bolt) = line.strip_prefix('⚡') else {
        return line.to_string();
    };
    let (before_colon, after_colon) = match after_bolt.split_once(':') {
        Some((before, after)) => (before, Some(after)),
        None => (after_bolt, None),
    };

    let mid = if let Some(open) = before_colon.find('[') {
        if let Some(close_rel) = before_colon[open..].find(']') {
            let close = open + close_rel;
            let pre = &before_colon[..open];
            let bracket = &before_colon[open..=close];
            let post = &before_colon[close + 1..];
            format!("{}{}{}", pre.green(), bracket.bright_green(), post.green())
        } else {
            before_colon.green().to_string()
        }
    } else {
        before_colon.green().to_string()
    };

    match after_colon {
        Some(cmd) => format!("⚡{mid}:{}", cmd.dimmed()),
        None => format!("⚡{mid}"),
    }
}

fn style_run_line(line: &str) -> String {
    use crate::repl::shell_run::{RUN_STDERR_BODY_PREFIX, RUN_STDIN_EOF_BODY_PREFIX};

    if line.is_empty() {
        return String::new();
    }

    // Channel / Ctrl+D markers: stripped from display; body painted red.
    // Check EOF first, then stderr (format never stacks both).
    if let Some(body) = line.strip_prefix(RUN_STDIN_EOF_BODY_PREFIX) {
        return if body.is_empty() {
            String::new()
        } else {
            body.red().to_string()
        };
    }
    if let Some(body) = line.strip_prefix(RUN_STDERR_BODY_PREFIX) {
        return if body.is_empty() {
            String::new()
        } else {
            body.red().to_string()
        };
    }

    if line.starts_with("✗ exit ") {
        // non-zero exit: red (yoyo print_run_result)
        format!("  {line}").red().to_string()
    } else if line.starts_with("✓ exit ") {
        // success exit: gray (including after Ctrl+D — body is red, exit stays dim)
        format!("  {line}").dimmed().to_string()
    } else if line.starts_with("💡 Command failed.") || line.starts_with("Command failed.") {
        // yoyo failure tip — dim, two-space indent
        format!("  {line}").dimmed().to_string()
    } else if line.starts_with("    ") {
        // failure re-preview under exit (yoyo 4-space indent already in line)
        line.dimmed().to_string()
    } else {
        // stdout body: white, no indent (yoyo streams flush-left)
        line.white().to_string()
    }
}

const HISTORY_DETAIL_INDENT: &str = "  ";
const HISTORY_DETAIL_BODY_INDENT: &str = "    ";

/// yoyo-aligned colors: bold white `Turn N`, green `You:`/`Agent:`, white body.
fn style_history_detail_line(line: &str) -> String {
    if line.is_empty() {
        return String::new();
    }

    if line.starts_with("Turn ") {
        return format!("{HISTORY_DETAIL_INDENT}{}", line.bold().white().to_string());
    }

    if let Some(rest) = line.strip_prefix("Total:") {
        return format!(
            "{HISTORY_DETAIL_INDENT}{}{}",
            "Total:".bold().white(),
            rest.white()
        );
    }

    let body = line.trim_start();
    for label in ["You:", "Agent:"] {
        if let Some(rest) = body.strip_prefix(label) {
            return format!(
                "{HISTORY_DETAIL_BODY_INDENT}{}{}",
                label.green(),
                rest.white()
            );
        }
    }

    // e.g. "(no assistant response)"
    format!("{HISTORY_DETAIL_BODY_INDENT}{}", body.dimmed())
}

fn is_init_success_line(line: &str) -> bool {
    line.starts_with("✓ Created GREATSAGE.md")
}

fn is_memory_success_line(line: &str) -> bool {
    line.starts_with("✓ Remembered:") || line.starts_with("✓ Forgot:")
}

fn is_memory_error_line(line: &str) -> bool {
    line.starts_with("error:")
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContextOutputMode {
    List,
    System,
    Files,
}

fn context_output_mode(output: &[String]) -> Option<ContextOutputMode> {
    let first = output.first()?;
    if first == "Project context files:" || first == "No project context files found." {
        Some(ContextOutputMode::List)
    } else if first == "System prompt sections:" || first == "System prompt is empty." {
        Some(ContextOutputMode::System)
    } else if first == "Files in this conversation:" || first == "(no files referenced yet)" {
        Some(ContextOutputMode::Files)
    } else {
        None
    }
}

fn is_context_file_entry(line: &str) -> bool {
    line.contains('(') && (line.ends_with(" line)") || line.ends_with(" lines)"))
}

fn style_context_section_header(line: &str) -> String {
    const INDENT: &str = "  ";
    if let Some(idx) = line.find("  (") {
        let header = &line[..idx];
        let meta = &line[idx..];
        return format!("{INDENT}{}{}", header.bold(), meta.dimmed());
    }
    format!("{INDENT}{}", line.bold())
}

fn style_context_list_line(line: &str) -> String {
    const INDENT2: &str = "  ";
    const INDENT4: &str = "    ";
    if line.is_empty() {
        return String::from(INDENT2);
    }
    if is_context_file_entry(line) {
        return format!("{INDENT4}{}", line.dimmed());
    }
    format!("{INDENT2}{}", line.dimmed())
}

fn style_context_system_line(line: &str) -> String {
    const INDENT2: &str = "  ";
    const INDENT4: &str = "    ";
    if line.is_empty() {
        return String::new();
    }
    if line == "System prompt sections:" {
        return format!("{INDENT2}{}", line.bold());
    }
    if line == "System prompt is empty." {
        return format!("{INDENT2}{}", line.dimmed());
    }
    if line.starts_with("Total:") {
        return format!("{INDENT2}{}", line.dimmed());
    }
    if line == "..." {
        return format!("{INDENT4}{}", line.dimmed());
    }
    if line.starts_with("# ") || line.starts_with("## ") {
        return style_context_section_header(line);
    }
    format!("{INDENT4}{}", line.dimmed())
}

fn style_context_files_line(line: &str) -> String {
    const INDENT2: &str = "  ";
    const INDENT4: &str = "    ";
    if line.is_empty() {
        return String::new();
    }
    if line == "Files in this conversation:" || line == "(no files referenced yet)" {
        return format!("{INDENT2}{}", line.dimmed());
    }
    format!("{INDENT4}{}", line.dimmed())
}

fn style_context_line(line: &str, mode: ContextOutputMode) -> String {
    match mode {
        ContextOutputMode::List => style_context_list_line(line),
        ContextOutputMode::System => style_context_system_line(line),
        ContextOutputMode::Files => style_context_files_line(line),
    }
}

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

fn style_init_line(line: &str) -> String {
    if is_init_success_line(line) {
        format!("  {line}").green().to_string()
    } else {
        format!("  {line}").dimmed().to_string()
    }
}

fn style_memory_line(line: &str) -> String {
    if is_memory_success_line(line) {
        format!("  {line}").green().to_string()
    } else if is_memory_error_line(line) {
        format!("  {line}").red().to_string()
    } else {
        format!("  {line}").dimmed().to_string()
    }
}

fn style_repl_output_line(line: &str) -> String {
    // `/bg` usage errors: flush-left red (no two-space REPL indent).
    if line.starts_with("Usage: /bg") {
        return line.red().to_string();
    }
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

/// Live `/run` body line (always uses run channel colors; no need for exit footer yet).
pub(super) fn write_repl_shell_stream_line(stdout: &mut MessageWriter<StdoutMessage>, line: &str) {
    let styled = style_run_line(line);
    stdout.write(StdoutMessage::from(format!("{styled}\n")));
}

pub(super) fn write_repl_handled_output(
    stdout: &mut MessageWriter<StdoutMessage>,
    output: &[String],
    detail: &[String],
) {
    let model_info = is_model_info_output(output);
    let tokens_output = is_tokens_output(output);
    let init_output = is_init_output(output);
    let memory_output = is_memory_output(output);
    let history_detail = is_history_detail_output(output);
    let cd_output = is_cd_output(output);
    // bg list / bg output before run: list rows embed `✗ exit`; output body may too.
    let bg_list_output = is_bg_list_output(output);
    let bg_output_output = is_bg_output_output(output);
    let run_output = is_run_output(output);
    let context_mode = context_output_mode(output);
    let mut in_session_totals = false;
    for line in output {
        if tokens_output && line == TOKENS_SESSION_TOTALS_HEADER {
            in_session_totals = true;
        }
        let styled = if model_info {
            style_model_info_line(line)
        } else if tokens_output {
            style_tokens_line(line, in_session_totals)
        } else if init_output {
            style_init_line(line)
        } else if memory_output {
            style_memory_line(line)
        } else if history_detail {
            style_history_detail_line(line)
        } else if cd_output {
            style_cd_line(line)
        } else if bg_list_output {
            style_bg_list_line(line)
        } else if bg_output_output {
            style_bg_output_line(line)
        } else if run_output {
            style_run_line(line)
        } else if is_bg_started_line(line) {
            style_bg_started_line(line)
        } else if let Some(mode) = context_mode {
            style_context_line(line, mode)
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
        // Blank line before dim bye, one newline after — process exits (no trailing blank + `>`).
        stdout.write(StdoutMessage::from(format!(
            "\n\n{}\n",
            <&str as colored::Colorize>::dimmed("  bye 👋")
        )));
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
