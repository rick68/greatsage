//! Session dashboard metrics for `/status`, `/tokens`, and `/cost`.
//!
//! ECS `TurnSummary` is primary; yoagent message totals are fallback when no turns exist.

use {
    super::{
        cost::{
            estimate_cost, extract_tool_call_summary, format_cache_stats, format_cost,
            format_tool_call_summary,
        },
        model_cmd::model_context_window,
        session_ops::block_on_session,
    },
    crate::{agents::CodingAgent, providers::Provider, utils::now_ms},
    bevy::utils::default,
    yoagent::{
        context::{ContextTracker, total_tokens},
        types::{AgentMessage, Content, Message as LlmMessage, Usage},
    },
};

/// One turn's token fields for pure aggregation tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TurnUsageRow {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// Session root metadata needed for dashboard display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMetaFields {
    pub started_at_ms: u64,
    pub cwd: String,
}

/// Projected active-context fields from Session ECS (`SessionContextStats`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionContextStatsFields {
    pub message_count: u32,
    pub context_used: u64,
    pub context_max: u64,
}

/// Sum token fields across projected turns.
pub fn aggregate_turn_usage(rows: &[TurnUsageRow]) -> (usize, Usage) {
    let mut usage = Usage::default();
    for row in rows {
        usage.input = usage.input.saturating_add(row.input_tokens);
        usage.output = usage.output.saturating_add(row.output_tokens);
        usage.cache_read = usage.cache_read.saturating_add(row.cache_read_tokens);
        usage.cache_write = usage.cache_write.saturating_add(row.cache_write_tokens);
    }
    (rows.len(), usage)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionDashboardSnapshot {
    pub turn_count: usize,
    pub message_count: usize,
    pub usage: Usage,
    pub context_used: u64,
    pub context_max: Option<u64>,
    /// True when yoagent messages contain a compaction marker or session input dwarfs active context.
    pub show_compaction_note: bool,
    pub started_at_ms: Option<u64>,
    pub cwd: Option<String>,
}

/// Active context via yoagent `ContextTracker` (retained for tests; `/tokens` uses `total_tokens`).
#[allow(dead_code)]
pub fn estimate_active_context(messages: &[AgentMessage]) -> u64 {
    let mut tracker = ContextTracker::new();
    for (idx, msg) in messages.iter().enumerate() {
        if let AgentMessage::Llm(LlmMessage::Assistant { usage, .. }) = msg {
            tracker.record_usage(usage, idx);
        }
    }
    tracker.estimate_context_tokens(messages) as u64
}

/// yoagent inserts this marker user message when `/compact` or auto-compact drops history.
pub fn messages_indicate_compaction(messages: &[AgentMessage]) -> bool {
    messages.iter().any(|msg| {
        let AgentMessage::Llm(LlmMessage::User { content, .. }) = msg else {
            return false;
        };
        content.iter().any(|block| {
            matches!(
                block,
                Content::Text { text } if text.contains("[Context compacted:")
            )
        })
    })
}

async fn yoagent_active_context(agent: &CodingAgent) -> (usize, u64, bool) {
    let guard = agent.lock().await;
    let messages = guard.messages();
    (
        messages.len(),
        total_tokens(messages) as u64,
        messages_indicate_compaction(messages),
    )
}

fn usage_total_tokens(usage: &Usage) -> u64 {
    usage
        .input
        .saturating_add(usage.output)
        .saturating_add(usage.cache_read)
        .saturating_add(usage.cache_write)
}

fn lifetime_usage_nonempty(lifetime: &Usage) -> bool {
    lifetime.input > 0 || lifetime.output > 0 || lifetime.cache_read > 0 || lifetime.cache_write > 0
}

fn context_max_from_stats_or_model(
    context_stats: Option<&SessionContextStatsFields>,
    model: &str,
    coding_agent: Option<&CodingAgent>,
) -> Option<u64> {
    context_stats
        .and_then(|stats| (stats.context_max > 0).then_some(stats.context_max))
        .or_else(|| coding_agent.map(CodingAgent::context_window).map(u64::from))
        .or_else(|| model_context_window(model))
}

/// Merge archived REPL lifetime totals with the active session's projected turns.
pub fn session_totals_usage(lifetime: &Usage, turn_rows: &[TurnUsageRow]) -> Usage {
    let (_, current) = aggregate_turn_usage(turn_rows);
    let mut totals = lifetime.clone();
    totals.input = totals.input.saturating_add(current.input);
    totals.output = totals.output.saturating_add(current.output);
    totals.cache_read = totals.cache_read.saturating_add(current.cache_read);
    totals.cache_write = totals.cache_write.saturating_add(current.cache_write);
    totals
}

/// yoyo `format_token_count` — e.g. `933`, `2.5k`, `1.0M`.
pub fn format_token_amount(tokens: u64) -> String {
    if tokens < 1_000 {
        format!("{tokens}")
    } else if tokens < 1_000_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    }
}

pub fn build_snapshot(
    turn_rows: &[TurnUsageRow],
    meta: Option<&SessionMetaFields>,
    model: &str,
    context_stats: Option<&SessionContextStatsFields>,
    coding_agent: Option<&CodingAgent>,
    runtime: &tokio::runtime::Runtime,
    lifetime_usage: &Usage,
) -> SessionDashboardSnapshot {
    let turn_count = if turn_rows.is_empty() {
        coding_agent
            .map(|agent| {
                block_on_session(runtime, async {
                    let guard = agent.lock().await;
                    guard.messages().len()
                })
            })
            .unwrap_or(0)
    } else {
        turn_rows.len()
    };

    // yoyo `session_total`: REPL-lifetime accumulator (ingest merges each API usage).
    // Fall back to ECS turn rows only before the first ingest write.
    let usage = if lifetime_usage_nonempty(lifetime_usage) {
        lifetime_usage.clone()
    } else if !turn_rows.is_empty() {
        session_totals_usage(&Usage::default(), turn_rows)
    } else {
        let tokens = coding_agent
            .map(|agent| {
                block_on_session(runtime, async {
                    let guard = agent.lock().await;
                    total_tokens(guard.messages()) as u64
                })
            })
            .unwrap_or(0);
        Usage {
            input: tokens,
            ..default()
        }
    };

    let (message_count, context_used, messages_compacted) = if let Some(stats) = context_stats {
        let messages_compacted = coding_agent
            .map(|agent| {
                block_on_session(runtime, async {
                    let guard = agent.lock().await;
                    messages_indicate_compaction(guard.messages())
                })
            })
            .unwrap_or(false);
        (
            stats.message_count as usize,
            stats.context_used,
            messages_compacted,
        )
    } else {
        coding_agent
            .map(|agent| block_on_session(runtime, yoagent_active_context(agent)))
            .unwrap_or((0, usage_total_tokens(&usage), false))
    };

    let context_max = context_max_from_stats_or_model(context_stats, model, coding_agent);

    let show_compaction_note =
        messages_compacted || show_compacted_context_note(usage.input, context_used);

    SessionDashboardSnapshot {
        turn_count,
        message_count,
        usage,
        context_used,
        context_max,
        show_compaction_note,
        started_at_ms: meta.map(|m| m.started_at_ms).filter(|ms| *ms > 0),
        cwd: meta
            .map(|m| m.cwd.as_str())
            .filter(|cwd| !cwd.is_empty())
            .map(str::to_owned),
    }
}

/// 20-column fill bar with percentage (yoyo `context_bar` — `<1%` when usage is non-zero but rounds to 0).
pub fn format_context_bar(used: u64, max: Option<u64>) -> String {
    const WIDTH: usize = 20;
    let pct = match max {
        Some(max) if max > 0 => (used as f64 / max as f64).min(1.0),
        _ => 0.0,
    };
    let filled = (pct * WIDTH as f64).round() as usize;
    let filled = filled.min(WIDTH);
    let pct_int = (pct * 100.0) as u32;
    let label = if used > 0 && pct_int == 0 {
        "<1%".to_string()
    } else {
        format!("{pct_int}%")
    };
    format!(
        "{}{} {label}",
        "█".repeat(filled),
        "░".repeat(WIDTH.saturating_sub(filled)),
    )
}

const TOKENS_COMPACTED_NOTE: &str =
    "(earlier messages were compacted to save space — session totals below show full usage)";

/// yoyo shows this when session API input dwarfs in-memory active context.
pub fn show_compacted_context_note(session_input: u64, context_used: u64) -> bool {
    session_input > context_used.saturating_add(1000)
}

/// yoyo warns when active context exceeds 75% of the window.
pub fn show_context_fill_warning(used: u64, max: Option<u64>) -> bool {
    match max {
        Some(max) if max > 0 => used as f64 / max as f64 > 0.75,
        _ => false,
    }
}

const TOKENS_CONTEXT_WARNING: &str = "⚠ Context is getting full. Consider /clear or /compact.";

/// Breakdown of what's consuming context tokens by category (yoyo `ContextBreakdown`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextBreakdown {
    pub system_estimate: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub tool_calls: usize,
    pub tool_results: usize,
    pub thinking: usize,
    pub total: usize,
}

/// chars/4 heuristic matching yoyo `estimate_tokens`.
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        (text.len() / 4).max(1)
    }
}

fn content_block_tokens(content: &[Content]) -> usize {
    content
        .iter()
        .map(|block| match block {
            Content::Text { text } => estimate_tokens(text),
            Content::Image { data, .. } => {
                let raw_bytes = data.len() * 3 / 4;
                (raw_bytes / 750).clamp(85, 16_000)
            }
            Content::Thinking { thinking, .. } => estimate_tokens(thinking),
            Content::ToolCall {
                name, arguments, ..
            } => estimate_tokens(name) + estimate_tokens(&arguments.to_string()) + 8,
        })
        .sum()
}

/// Analyze messages to produce a per-category token breakdown.
pub fn context_breakdown(messages: &[AgentMessage], system_prompt: &str) -> ContextBreakdown {
    let system_estimate = estimate_tokens(system_prompt);

    let mut user_messages = 0usize;
    let mut assistant_messages = 0usize;
    let mut tool_calls = 0usize;
    let mut tool_results = 0usize;
    let mut thinking = 0usize;

    for msg in messages {
        match msg {
            AgentMessage::Llm(m) => match m {
                LlmMessage::User { content, .. } => {
                    user_messages += content_block_tokens(content) + 4;
                }
                LlmMessage::Assistant { content, .. } => {
                    for block in content {
                        match block {
                            Content::Text { text } => {
                                assistant_messages += estimate_tokens(text);
                            }
                            Content::Thinking { thinking: t, .. } => {
                                thinking += estimate_tokens(t);
                            }
                            Content::ToolCall {
                                name, arguments, ..
                            } => {
                                tool_calls += estimate_tokens(name)
                                    + estimate_tokens(&arguments.to_string())
                                    + 8;
                            }
                            Content::Image { data, .. } => {
                                let raw_bytes = data.len() * 3 / 4;
                                assistant_messages += (raw_bytes / 750).clamp(85, 16_000);
                            }
                        }
                    }
                    assistant_messages += 4;
                }
                LlmMessage::ToolResult {
                    content, tool_name, ..
                } => {
                    tool_results += content_block_tokens(content) + estimate_tokens(tool_name) + 8;
                }
            },
            AgentMessage::Extension(ext) => {
                user_messages += estimate_tokens(&ext.data.to_string()) + 4;
            }
        }
    }

    let total =
        system_estimate + user_messages + assistant_messages + tool_calls + tool_results + thinking;

    ContextBreakdown {
        system_estimate,
        user_messages,
        assistant_messages,
        tool_calls,
        tool_results,
        thinking,
        total,
    }
}

/// Format a context breakdown table with percentages (plain text; terminal dims).
pub fn format_context_breakdown(breakdown: &ContextBreakdown) -> Vec<String> {
    let total = breakdown.total.max(1);
    let categories: &[(&str, usize)] = &[
        ("system prompt", breakdown.system_estimate),
        ("user messages", breakdown.user_messages),
        ("assistant", breakdown.assistant_messages),
        ("tool calls", breakdown.tool_calls),
        ("tool results", breakdown.tool_results),
        ("thinking", breakdown.thinking),
    ];
    let mut lines = vec!["Context breakdown:".to_string()];
    for &(label, value) in categories {
        if value == 0 {
            continue;
        }
        let pct = (value as f64 / total as f64) * 100.0;
        let tok_str = format_token_amount(value as u64);
        lines.push(format!("    {label:<16} {tok_str:>7} tokens  ({pct:.0}%)"));
    }
    () = lines.push(format!("    {}", "─".repeat(38)));
    () = lines.push(format!(
        "    {:<16} {:>7} tokens",
        "total",
        format_token_amount(total as u64),
    ));

    let tool_pct = (breakdown.tool_results as f64 / total as f64) * 100.0;
    if tool_pct > 50.0 {
        lines.push(format!(
            "  💡 Tool results are {tool_pct:.0}% of context — consider /compact."
        ));
    }

    lines
}

/// Estimate how many more turns fit before hitting the context limit.
pub fn estimate_remaining_turns(
    messages: &[AgentMessage],
    max_context: u64,
) -> Option<(usize, f64)> {
    if max_context == 0 {
        return None;
    }

    let turn_count = messages
        .iter()
        .filter(|msg| matches!(msg, AgentMessage::Llm(LlmMessage::Assistant { .. })))
        .count();

    if turn_count < 2 {
        return None;
    }

    let context_used = total_tokens(messages) as u64;
    if context_used == 0 {
        return None;
    }

    let avg_per_turn = context_used as f64 / turn_count as f64;
    let remaining_capacity = max_context.saturating_sub(context_used);
    let remaining_turns = (remaining_capacity as f64 / avg_per_turn).floor() as usize;

    Some((remaining_turns, avg_per_turn))
}

/// Plain-text remaining-turns line (terminal applies yellow/red).
pub fn format_remaining_turns(remaining: usize, avg_per_turn: f64) -> String {
    let avg_str = format_token_amount(avg_per_turn as u64);
    if remaining == 0 {
        format!("⚠ Context nearly full (~{avg_str}/turn avg)")
    } else if remaining <= 3 {
        format!(
            "~{remaining} {} remaining (~{avg_str}/turn avg)",
            if remaining == 1 { "turn" } else { "turns" }
        )
    } else {
        format!("~{remaining} turns remaining (~{avg_str}/turn avg)")
    }
}

/// yoyo-shaped `/tokens` output lines.
pub fn tokens_output_lines(
    snap: &SessionDashboardSnapshot,
    messages: &[AgentMessage],
    system_prompt: &str,
    provider: Provider,
    model: &str,
) -> Vec<String> {
    let current_line = match snap.context_max {
        Some(max) => format!(
            "  current:     {} / {} tokens",
            format_token_amount(snap.context_used),
            format_token_amount(max),
        ),
        None => format!(
            "  current:     {} / unknown tokens",
            format_token_amount(snap.context_used),
        ),
    };

    let mut lines = vec![
        "Active context:".to_string(),
        format!("  messages:    {}", snap.message_count),
        current_line,
        format!(
            "  {}",
            format_context_bar(snap.context_used, snap.context_max)
        ),
    ];

    if let Some(max) = snap.context_max.filter(|max| *max > 0)
        && let Some((remaining, avg)) = estimate_remaining_turns(messages, max)
    {
        () = lines.push(format!("  {}", format_remaining_turns(remaining, avg)));
    }

    if !messages.is_empty() {
        let breakdown = context_breakdown(messages, system_prompt);
        () = lines.push(String::new());
        () = lines.extend(format_context_breakdown(&breakdown));
    }

    let tool_summary = extract_tool_call_summary(messages);
    let tool_table = format_tool_call_summary(&tool_summary);
    if !tool_table.is_empty() {
        () = lines.push(String::new());
        () = lines.extend(tool_table.lines().map(str::to_owned));
    }

    if snap.show_compaction_note {
        () = lines.push(format!("  {TOKENS_COMPACTED_NOTE}"));
    }
    if show_context_fill_warning(snap.context_used, snap.context_max) {
        () = lines.push(format!("  {TOKENS_CONTEXT_WARNING}"));
    }

    () = lines.push(String::new());
    () = lines.extend([
        "Session totals (all API calls):".to_string(),
        format!(
            "  input:       {} tokens",
            format_token_amount(snap.usage.input)
        ),
        format!(
            "  output:      {} tokens",
            format_token_amount(snap.usage.output)
        ),
        format!(
            "  cache read:  {} tokens",
            format_token_amount(snap.usage.cache_read)
        ),
        format!(
            "  cache write: {} tokens",
            format_token_amount(snap.usage.cache_write)
        ),
    ]);

    if let Some(cache_line) = format_cache_stats(&snap.usage) {
        () = lines.push(format!("  {cache_line}"));
    }
    if let Some(cost) = estimate_cost(&snap.usage, provider, model) {
        () = lines.push(format!("  est. cost:   {}", format_cost(cost)));
    }

    lines
}

pub fn format_elapsed(started_at_ms: Option<u64>) -> Option<String> {
    let started = started_at_ms.filter(|ms| *ms > 0)?;
    let elapsed_ms = now_ms().saturating_sub(started);
    let total_secs = elapsed_ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins > 0 {
        Some(format!("{mins}m {secs}s"))
    } else {
        Some(format!("{secs}s"))
    }
}

pub fn format_context_fill(used: u64, max: Option<u64>) -> String {
    match max {
        Some(max) if max > 0 => {
            let pct = (used as f64 / max as f64 * 100.0).min(100.0);
            format!("{used} / {max} ({pct:.1}%)")
        }
        _ => format!("{used} (max unknown)"),
    }
}
