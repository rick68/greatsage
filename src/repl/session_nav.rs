//! Pure helpers for session navigation: history previews, search, export markdown.
//!
//! No Bevy / REPL I/O — handlers live in [`super::commands_session_nav`].

use {
    std::collections::HashMap,
    yoagent::types::{AgentMessage, Content, Message},
};

pub const DEFAULT_EXPORT_PATH: &str = "conversation.md";

/// Truncate to `max_chars` codepoints, appending `…` when shortened.
pub fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    let trimmed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    () = out.push('…');
    out
}

fn extract_user_text(content: &[Content]) -> String {
    content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn message_text(msg: &AgentMessage) -> String {
    match msg {
        AgentMessage::Llm(Message::User { content, .. }) => extract_user_text(content),
        AgentMessage::Llm(Message::Assistant { content, .. }) => {
            let mut parts = Vec::new();
            for c in content {
                match c {
                    Content::Text { text } if !text.is_empty() => parts.push(text.as_str()),
                    Content::ToolCall { name, .. } => parts.push(name.as_str()),
                    _ => {}
                }
            }
            parts.join(" ")
        }
        AgentMessage::Llm(Message::ToolResult {
            tool_name, content, ..
        }) => {
            let text = extract_user_text(content);
            format!("{tool_name} {text}")
        }
        AgentMessage::Extension(ext) => ext.role.clone(),
    }
}

/// Summarize a message for `/history` display: `(role, preview)`.
pub fn summarize_message(msg: &AgentMessage) -> (&'static str, String) {
    match msg {
        AgentMessage::Llm(Message::User { content, .. }) => {
            let text = extract_user_text(content);
            ("user", truncate_with_ellipsis(&text, 80))
        }
        AgentMessage::Llm(Message::Assistant { content, .. }) => {
            let mut parts = Vec::new();
            let mut tool_calls = 0usize;
            for c in content {
                match c {
                    Content::Text { text } if !text.is_empty() => {
                        // Whitespace-only text collapses to "" — do not push empty
                        // parts or join leaves a leading "  " before `→tool`.
                        let t = truncate_with_ellipsis(text, 60);
                        if !t.is_empty() {
                            parts.push(t);
                        }
                    }
                    Content::ToolCall { name, .. } => {
                        tool_calls += 1;
                        if tool_calls <= 3 {
                            parts.push(format!("→{name}"));
                        }
                    }
                    _ => {}
                }
            }
            if tool_calls > 3 {
                parts.push(format!("(+{} more tools)", tool_calls - 3));
            }
            let preview = if parts.is_empty() {
                String::from("(empty)")
            } else {
                parts.join("  ")
            };
            ("assistant", preview)
        }
        AgentMessage::Llm(Message::ToolResult {
            tool_name,
            is_error,
            ..
        }) => {
            let status = if *is_error { "✗" } else { "✓" };
            ("tool", format!("{tool_name} {status}"))
        }
        AgentMessage::Extension(ext) => ("ext", truncate_with_ellipsis(&ext.role, 60)),
    }
}

/// Case-insensitive substring search. Returns `(1-based index, role, preview)`.
pub fn search_messages(messages: &[AgentMessage], query: &str) -> Vec<(usize, String, String)> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for (i, msg) in messages.iter().enumerate() {
        let text = message_text(msg);
        if !text.to_lowercase().contains(&query_lower) {
            continue;
        }
        let (role, _) = summarize_message(msg);
        let preview = truncate_with_ellipsis(&text, 80);
        () = results.push((i + 1, role.to_string(), preview));
    }

    results
}

fn count_tool_calls(content: &[Content]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for block in content {
        if let Content::ToolCall { name, .. } = block {
            *counts.entry(name.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn format_tool_summary(counts: &HashMap<String, usize>) -> String {
    if counts.is_empty() {
        return "no tool calls".to_string();
    }
    let mut entries: Vec<_> = counts.iter().collect();
    () = entries.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    entries
        .iter()
        .map(|(name, count)| format!("{name} ×{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_token_count(tokens: u64) -> String {
    if tokens < 1_000 {
        format!("{tokens}")
    } else if tokens < 1_000_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    }
}

/// Numbered list lines for `/history` (empty-state when no messages).
pub fn history_list_lines(messages: &[AgentMessage]) -> Vec<String> {
    if messages.is_empty() {
        return vec![String::from("(no messages in conversation)")];
    }
    let mut lines = vec![format!("Conversation ({} messages):", messages.len())];
    for (i, msg) in messages.iter().enumerate() {
        let (role, preview) = summarize_message(msg);
        () = lines.push(format!("  {:>3}. [{role}] {preview}", i + 1));
    }
    lines
}

/// Per-turn breakdown for `/history detail`.
///
/// Plain text lines (styling applied in `terminal::style_history_detail_line`):
/// leading blank, then `Turn N` / indented `You:` / `Agent:` blocks, then `Total:`.
pub fn history_detail_lines(messages: &[AgentMessage]) -> Vec<String> {
    if messages.is_empty() {
        return vec![String::from("(no messages in conversation)")];
    }

    // turn = user content + following assistant/tool messages until next user
    let mut turns: Vec<(Option<&[Content]>, Vec<&Message>)> = Vec::new();
    for msg in messages {
        match msg {
            AgentMessage::Llm(m @ Message::User { content, .. }) => {
                () = turns.push((Some(content), vec![m]));
            }
            AgentMessage::Llm(m) => {
                if let Some(turn) = turns.last_mut() {
                    () = turn.1.push(m);
                } else {
                    () = turns.push((None, vec![m]));
                }
            }
            AgentMessage::Extension(_) => {}
        }
    }

    // Leading blank line (yoyo prints an empty line before the first turn).
    let mut lines = vec![String::new()];
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;

    for (turn_idx, (user_content, msgs)) in turns.iter().enumerate() {
        let turn_num = turn_idx + 1;
        let user_preview = if let Some(content) = user_content {
            let text = extract_user_text(content);
            if text.is_empty() {
                String::from("(no text)")
            } else {
                format!("\"{}\"", truncate_with_ellipsis(&text, 60))
            }
        } else {
            String::from("(system)")
        };

        let mut all_tool_counts: HashMap<String, usize> = HashMap::new();
        let mut turn_input: u64 = 0;
        let mut turn_output: u64 = 0;
        let mut has_assistant = false;

        for m in msgs {
            if let Message::Assistant { content, usage, .. } = m {
                has_assistant = true;
                turn_input += usage.input + usage.cache_read + usage.cache_write;
                turn_output += usage.output;
                for (name, count) in count_tool_calls(content) {
                    *all_tool_counts.entry(name).or_insert(0) += count;
                }
            }
        }

        total_input_tokens += turn_input;
        total_output_tokens += turn_output;

        lines.push(format!("Turn {turn_num}"));
        lines.push(format!("  You:   {user_preview}"));
        if has_assistant {
            let tool_total: usize = all_tool_counts.values().sum();
            let tool_summary = format_tool_summary(&all_tool_counts);
            let call_word = if tool_total == 1 {
                "tool call"
            } else {
                "tool calls"
            };
            lines.push(format!(
                "  Agent: {tool_total} {call_word}, {tool_summary}, {} tok in / {} tok out",
                format_token_count(turn_input),
                format_token_count(turn_output),
            ));
        } else {
            () = lines.push(String::from("  (no assistant response)"));
        }
        () = lines.push(String::new());
    }

    let total = total_input_tokens + total_output_tokens;
    let turn_word = if turns.len() == 1 { "turn" } else { "turns" };
    lines.push(format!(
        "Total: {} {turn_word}, ~{} tokens ({} in + {} out)",
        turns.len(),
        format_token_count(total),
        format_token_count(total_input_tokens),
        format_token_count(total_output_tokens),
    ));
    lines
}

/// Format conversation as readable markdown (yoyo-aligned).
pub fn format_conversation_markdown(messages: &[AgentMessage]) -> String {
    let mut out = String::from("# Conversation\n\n");

    for msg in messages {
        match msg {
            AgentMessage::Llm(Message::User { content, .. }) => {
                out.push_str("## User\n\n");
                for c in content {
                    if let Content::Text { text } = c {
                        () = out.push_str(text);
                        () = out.push_str("\n\n");
                    }
                }
            }
            AgentMessage::Llm(Message::Assistant { content, .. }) => {
                out.push_str("## Assistant\n\n");
                for c in content {
                    match c {
                        Content::Text { text } if !text.is_empty() => {
                            () = out.push_str(text);
                            () = out.push_str("\n\n");
                        }
                        Content::Thinking { thinking, .. } if !thinking.is_empty() => {
                            () = out.push_str("*Thinking:*\n\n> ");
                            () = out.push_str(&thinking.replace('\n', "\n> "));
                            () = out.push_str("\n\n");
                        }
                        _ => {}
                    }
                }
            }
            AgentMessage::Llm(Message::ToolResult {
                tool_name, content, ..
            }) => {
                out.push_str(&format!("### Tool: {tool_name}\n\n"));
                let text = extract_user_text(content);
                if !text.is_empty() {
                    () = out.push_str("```\n");
                    () = out.push_str(&text);
                    () = out.push_str("\n```\n\n");
                }
            }
            AgentMessage::Extension(_) => {}
        }
    }

    out
}

/// Parse bookmark name from `/mark` or `/jump` args (remainder after command token).
pub fn parse_bookmark_name(args: &str) -> Option<String> {
    let name = args.trim();
    if name.is_empty() {
        None
    } else {
        Some(String::from(name))
    }
}

/// Resolve `/export` path; default [`DEFAULT_EXPORT_PATH`] when empty.
pub fn parse_export_path(args: &str) -> &str {
    let path = args.trim();
    if path.is_empty() {
        DEFAULT_EXPORT_PATH
    } else {
        path
    }
}
