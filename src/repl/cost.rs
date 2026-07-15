//! Session cost estimation — yoyo-style static rates via [`super::native_pricing`].

use {
    super::native_pricing::{self, PerMTok},
    crate::providers::Provider,
    std::collections::HashMap,
    yoagent::types::{AgentMessage, Message, Usage},
};

/// yoyo `format_token_count` — e.g. `933`, `2.5k`, `1.0M`.
fn format_token_count(tokens: u64) -> String {
    if tokens < 1_000 {
        format!("{tokens}")
    } else if tokens < 1_000_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    }
}

fn resolve_pricing(model: &str) -> Option<PerMTok> {
    native_pricing::native_model_pricing(model)
}

/// Lines for `/model info` — static estimated rates (yoyo-style rules).
pub fn format_pricing_lines(_provider: Provider, model: &str) -> Vec<String> {
    native_pricing::format_native_pricing_lines(model)
}

pub fn cost_breakdown(
    usage: &Usage,
    _provider: Provider,
    model: &str,
) -> Option<(f64, f64, f64, f64)> {
    let (input_per_m, cache_write_per_m, cache_read_per_m, output_per_m) = resolve_pricing(model)?;

    let input_cost = usage.input as f64 * input_per_m / 1_000_000.0;
    let cache_write_cost = usage.cache_write as f64 * cache_write_per_m / 1_000_000.0;
    let cache_read_cost = usage.cache_read as f64 * cache_read_per_m / 1_000_000.0;
    let output_cost = usage.output as f64 * output_per_m / 1_000_000.0;

    Some((input_cost, cache_write_cost, cache_read_cost, output_cost))
}

pub fn estimate_cost(usage: &Usage, provider: Provider, model: &str) -> Option<f64> {
    let (input_cost, cw_cost, cr_cost, output_cost) = cost_breakdown(usage, provider, model)?;
    Some(input_cost + cw_cost + cr_cost + output_cost)
}

/// Format a cost in USD for display (yoyo `format_cost`).
pub fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${cost:.4}")
    } else if cost < 1.0 {
        format!("${cost:.3}")
    } else {
        format!("${cost:.2}")
    }
}

fn cache_hit_rate(usage: &Usage) -> f64 {
    let denom = usage
        .input
        .saturating_add(usage.cache_read)
        .saturating_add(usage.cache_write);
    if denom == 0 {
        return 0.0;
    }
    usage.cache_read as f64 / denom as f64
}

/// yoyo `format_cache_stats` — `None` when no cache activity.
pub fn format_cache_stats(usage: &Usage) -> Option<String> {
    if usage.cache_read == 0 && usage.cache_write == 0 {
        return None;
    }
    let pct = (cache_hit_rate(usage) * 100.0) as u32;
    Some(format!(
        "Cache: {pct}% hit rate ({} read, {} written)",
        format_token_count(usage.cache_read),
        format_token_count(usage.cache_write),
    ))
}

/// Per-turn cost extracted from assistant messages (yoyo `TurnCost`).
#[derive(Debug, Clone, PartialEq)]
pub struct TurnCost {
    pub turn_number: usize,
    pub usage: Usage,
    pub cost_usd: Option<f64>,
}

pub fn extract_turn_costs(
    messages: &[AgentMessage],
    provider: Provider,
    model: &str,
) -> Vec<TurnCost> {
    let mut turns = Vec::new();
    let mut turn_number = 0;
    for msg in messages {
        if let AgentMessage::Llm(Message::Assistant { usage, .. }) = msg {
            turn_number += 1;
            turns.push(TurnCost {
                turn_number,
                usage: usage.clone(),
                cost_usd: estimate_cost(usage, provider, model),
            });
        }
    }
    turns
}

/// Fixed-width columns for the per-turn cost table (header / rows / Total must match).
///
/// ```text
///  Turn   Input   Output       Cost
///     1    8.0k       43    $0.0016
/// ─────────────────────────────────
/// Total    8.0k       43    $0.0016
/// ```
fn format_turn_cost_row(label: &str, input: &str, output: &str, cost: &str) -> String {
    // label: 5 (right) · input: 7 · output: 7 · cost: 10 (right-aligned $ amounts)
    format!("    {label:>5} {input:>7} {output:>7}  {cost:>10}")
}

pub fn format_turn_costs(costs: &[TurnCost]) -> String {
    if costs.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push("  Per-turn breakdown:".to_string());
    lines.push(format_turn_cost_row("Turn", "Input", "Output", "Cost"));

    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    let mut total_cost: f64 = 0.0;
    let mut has_cost = false;

    for tc in costs {
        total_input = total_input.saturating_add(tc.usage.input);
        total_output = total_output.saturating_add(tc.usage.output);
        let cost_str = match tc.cost_usd {
            Some(c) => {
                has_cost = true;
                total_cost += c;
                format_cost(c)
            }
            None => "—".to_string(),
        };
        lines.push(format_turn_cost_row(
            &tc.turn_number.to_string(),
            &format_token_count(tc.usage.input),
            &format_token_count(tc.usage.output),
            &cost_str,
        ));
    }

    lines.push(String::from("    ─────────────────────────────────"));
    let total_cost_str = if has_cost {
        format_cost(total_cost)
    } else {
        String::from("—")
    };
    () = lines.push(format_turn_cost_row(
        "Total",
        &format_token_count(total_input),
        &format_token_count(total_output),
        &total_cost_str,
    ));

    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallSummary {
    pub name: String,
    pub calls: usize,
    pub errors: usize,
}

pub fn extract_tool_call_summary(messages: &[AgentMessage]) -> Vec<ToolCallSummary> {
    let mut counts: HashMap<String, (usize, usize)> = HashMap::new();

    for msg in messages {
        let AgentMessage::Llm(Message::ToolResult {
            tool_name,
            is_error,
            ..
        }) = msg
        else {
            continue;
        };
        let entry = counts.entry(tool_name.clone()).or_insert((0, 0));
        entry.0 += 1;
        if *is_error {
            entry.1 += 1;
        }
    }

    let mut result: Vec<ToolCallSummary> = counts
        .into_iter()
        .map(|(name, (calls, errors))| ToolCallSummary {
            name,
            calls,
            errors,
        })
        .collect();

    result.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.name.cmp(&b.name)));
    result
}

fn pluralize<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

pub fn format_tool_call_summary(summary: &[ToolCallSummary]) -> String {
    if summary.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    lines.push("  Tool usage:".to_string());

    let max_name_len = summary.iter().map(|s| s.name.len()).max().unwrap_or(0);
    let total_calls: usize = summary.iter().map(|s| s.calls).sum();
    let total_errors: usize = summary.iter().map(|s| s.errors).sum();

    for s in summary {
        let error_str = if s.errors > 0 {
            format!(" ({} {})", s.errors, pluralize(s.errors, "error", "errors"))
        } else {
            String::new()
        };
        lines.push(format!(
            "    {:<width$}  {:>3} {}{}",
            s.name,
            s.calls,
            pluralize(s.calls, "call", "calls"),
            error_str,
            width = max_name_len,
        ));
    }

    let total_error_str = if total_errors > 0 {
        format!(
            " ({} {})",
            total_errors,
            pluralize(total_errors, "error", "errors")
        )
    } else {
        String::new()
    };
    lines.push(format!(
        "    {:<width$}  {:>3} total{}",
        "—",
        total_calls,
        total_error_str,
        width = max_name_len,
    ));

    lines.join("\n")
}

/// yoyo-shaped `/cost` output lines (plain text; REPL terminal adds dim indent).
pub fn cost_output_lines(
    usage: &Usage,
    provider: Provider,
    model: &str,
    messages: &[AgentMessage],
) -> Vec<String> {
    let Some(total) = estimate_cost(usage, provider, model) else {
        return vec![format!(
            "Cost estimation not available for model '{model}'."
        )];
    };

    let mut lines = vec![
        format!("Session cost: {}", format_cost(total)),
        format!(
            "  {} in / {} out",
            format_token_count(usage.input),
            format_token_count(usage.output),
        ),
    ];

    if usage.cache_read > 0 || usage.cache_write > 0 {
        lines.push(format!(
            "  cache: {} read / {} write",
            format_token_count(usage.cache_read),
            format_token_count(usage.cache_write),
        ));
        if let Some(cache_line) = format_cache_stats(usage) {
            lines.push(format!("  {cache_line}"));
        }
    }

    if let Some((input_cost, cw_cost, cr_cost, output_cost)) =
        cost_breakdown(usage, provider, model)
    {
        () = lines.push(String::new());
        () = lines.push("  Breakdown:".to_string());
        // Labels padded to same width so `$` amounts share one column.
        const BREAKDOWN_LABEL_W: usize = 12;
        () = lines.push(format!(
            "    {:width$} {}",
            "input:",
            format_cost(input_cost),
            width = BREAKDOWN_LABEL_W
        ));
        () = lines.push(format!(
            "    {:width$} {}",
            "output:",
            format_cost(output_cost),
            width = BREAKDOWN_LABEL_W
        ));
        if cw_cost > 0.0 {
            () = lines.push(format!(
                "    {:width$} {}",
                "cache write:",
                format_cost(cw_cost),
                width = BREAKDOWN_LABEL_W
            ));
        }
        if cr_cost > 0.0 {
            () = lines.push(format!(
                "    {:width$} {}",
                "cache read:",
                format_cost(cr_cost),
                width = BREAKDOWN_LABEL_W
            ));
        }
    }

    let turn_costs = extract_turn_costs(messages, provider, model);
    let turn_table = format_turn_costs(&turn_costs);
    if !turn_table.is_empty() {
        () = lines.push(String::new());
        () = lines.extend(turn_table.lines().map(str::to_owned));
    }

    let tool_summary = extract_tool_call_summary(messages);
    let tool_table = format_tool_call_summary(&tool_summary);
    if !tool_table.is_empty() {
        () = lines.push(String::new());
        () = lines.extend(tool_table.lines().map(str::to_owned));
    }

    lines
}
