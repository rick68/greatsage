use {
    crate::tui::{
        core::{ClickAction, ResponseBlock},
        renderer::widgets::{thinking, tool_call},
    },
    ratatui::{
        prelude::Stylize,
        text::{Line, Span},
    },
};

/// Renders a response block into lines and corresponding click actions.
pub fn render_response_lines(
    resp: &ResponseBlock,
    selected: bool,
    spinner: &[&str],
) -> (Vec<Line<'static>>, Vec<ClickAction>) {
    let mut content: Vec<Line<'static>> = Vec::new();
    let mut actions: Vec<ClickAction> = Vec::new();

    for (ti, tb) in resp.thinkings.iter().enumerate() {
        content.push(thinking::render_thinking_header(tb, spinner));
        actions.push(ClickAction::ToggleThinking(ti));
        if tb.streaming || tb.expanded {
            for line in &tb.lines {
                let mut spans = vec![Span::from("  │ ").dim()];
                spans.extend(line.spans.iter().cloned());
                content.push(Line::from(spans));
                actions.push(ClickAction::Select);
            }
            if !tb.streaming {
                content.push(Line::from(Span::from("  └─").dim()));
                actions.push(ClickAction::Select);
            }
        }
    }

    for tc in &resp.tool_calls {
        content.push(tool_call::render_tool_call(tc, spinner));
        actions.push(ClickAction::Select);
    }

    if !resp.text_lines.is_empty() || resp.text_streaming {
        if !resp.thinkings.is_empty() || !resp.tool_calls.is_empty() {
            content.push(Line::from(""));
            actions.push(ClickAction::Select);
        }
        for line in &resp.text_lines {
            content.push(line.clone());
            actions.push(ClickAction::Select);
        }
    }

    if selected {
        let lines: Vec<Line<'static>> = content
            .into_iter()
            .map(|line| {
                let mut spans = vec![Span::from("▌ ").light_blue()];
                spans.extend(line.spans);
                Line::from(spans)
            })
            .collect();
        let mut lines = lines;
        lines.push(Line::from(
            Span::from("▌─────────────────────────────────────────────────").light_blue(),
        ));
        actions.push(ClickAction::Select);
        (lines, actions)
    } else {
        let mut lines = content;
        lines.push(Line::from(
            Span::from("──────────────────────────────────────────────────").dark_gray(),
        ));
        actions.push(ClickAction::Select);
        (lines, actions)
    }
}
