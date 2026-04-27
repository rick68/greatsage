//! Renderer for a complete [`ResponseBlock`].
//!
//! [`render_response_lines`] produces two parallel vectors:
//! * `Vec<Line<'static>>` — all visual lines for the block
//! * `Vec<ClickAction>`   — what a left-click on each line should do
//!
//! # Visual structure
//!
//! ```text
//! 💭 Thinking  ▶  [1.2s · 340 tokens]  (t)    ← ClickAction::ToggleThinking(0)
//!  …body shown with dark-purple background…    ← ClickAction::Select (tinted bg)
//! 🔧 bash(ls -la)  ✅  0.3s                    ← ClickAction::Select
//!                                              ← (blank separator)
//! The agent's markdown reply goes here.        ← ClickAction::Select
//! ──────────────────────────────────────────   ← divider (gray / cyan-rule when selected)
//! ```
//!
//! When `selected = true`, every line without its own background gets a dark-blue
//! tint; the divider becomes a cyan rule. No gutter prefix is added, so column
//! positions never shift.
//!
//! [`ResponseBlock`]: crate::tui::core::ResponseBlock

use {
    crate::tui::{
        core::{ClickAction, ResponseBlock},
        renderer::widgets::{thinking, tool_call},
    },
    ratatui::{
        style::{Color, Style, Stylize},
        text::{Line, Span},
    },
};

/// Renders `resp` into a flat list of lines and parallel click actions.
///
/// # Arguments
///
/// * `resp` — the response block to render (may still be streaming)
/// * `selected` — when `true`, draw the cyan selection gutter on every line
/// * `spinner` — the current spinner frames array (passed through for live animations)
pub fn render_response_lines(
    resp: &ResponseBlock,
    selected: bool,
    spinner: &[&str],
) -> (Vec<Line<'static>>, Vec<ClickAction>) {
    let mut content: Vec<Line<'static>> = Vec::new();
    let mut actions: Vec<ClickAction> = Vec::new();

    // ── Thinking blocks ───────────────────────────────────────────────────────
    for (ti, tb) in resp.thinkings.iter().enumerate() {
        // The header row (▶/▼ or live spinner) is clickable; clicking it toggles
        // this specific thinking block without affecting others.
        () = content.push(thinking::render_thinking_header(tb, spinner));
        () = actions.push(ClickAction::ToggleThinking(ti));

        // Body lines are only rendered while streaming or when expanded.
        if tb.streaming || tb.expanded {
            let thinking_bg = Style::default().bg(Color::Rgb(38, 32, 58));
            for line in &tb.lines {
                let mut styled = line.clone();
                styled.style = thinking_bg;
                () = content.push(styled);
                () = actions.push(ClickAction::Select);
            }
        }
    }

    // ── Tool call entries ─────────────────────────────────────────────────────
    for tc in &resp.tool_calls {
        () = content.push(tool_call::render_tool_call(tc, spinner));
        () = actions.push(ClickAction::Select);
    }

    // ── Text reply ────────────────────────────────────────────────────────────
    if !resp.text_lines.is_empty() || resp.text_streaming {
        // Insert a blank separator when there was preceding thinking or tool output.
        if !resp.thinkings.is_empty() || !resp.tool_calls.is_empty() {
            () = content.push(Line::from(""));
            () = actions.push(ClickAction::Select);
        }
        for line in &resp.text_lines {
            () = content.push(line.clone());
            () = actions.push(ClickAction::Select);
        }
    }

    // ── Bottom divider + optional selection highlight ─────────────────────────
    if selected {
        // Apply a subtle background tint to every content line instead of a gutter prefix.
        let selection_bg = Style::default().bg(Color::Rgb(25, 45, 65));
        let lines: Vec<Line<'static>> = content
            .into_iter()
            .map(|mut line| {
                // Only apply selection bg if the line doesn't already have a
                // background (e.g. the thinking block body has its own tint).
                if line.style.bg.is_none() {
                    line.style = line.style.patch(selection_bg);
                }
                line
            })
            .collect();
        let mut lines = lines;
        // Cyan closing rule — marks the end of the selected block.
        () = lines.push(Line::from(
            Span::from("──────────────────────────────────────────────────").light_blue(),
        ));
        () = actions.push(ClickAction::Select);
        (lines, actions)
    } else {
        let mut lines = content;
        // Plain dim gray divider — separates this block from the next.
        () = lines.push(Line::from(
            Span::from("──────────────────────────────────────────────────").dark_gray(),
        ));
        () = actions.push(ClickAction::Select);
        (lines, actions)
    }
}
