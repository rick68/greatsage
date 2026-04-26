//! Renderer for a single [`ThinkingBlock`] header line.
//!
//! The header is the only visible row when a thinking block is collapsed.
//! It shows either:
//! * A live spinner + elapsed time while the LLM is still producing thoughts.
//! * A ▶/▼ triangle, elapsed time, and token count once streaming finishes.
//!
//! Clicking the header (via [`ClickAction::ToggleThinking`]) expands/collapses
//! the body without touching any other blocks.
//!
//! [`ThinkingBlock`]: crate::tui::core::ThinkingBlock
//! [`ClickAction::ToggleThinking`]: crate::tui::core::ClickAction::ToggleThinking

use {
    crate::tui::core::ThinkingBlock,
    ratatui::{
        style::Stylize,
        text::{Line, Span},
    },
};

/// Renders the single-line header for `tb`.
///
/// # Streaming state (`tb.streaming = true`)
///
/// ```text
/// 💭 Thinking  ⣾  1.4s
/// ```
/// The spinner frame is derived from `elapsed_ms / 100 % spinner.len()` so it
/// animates at ~10 fps when the spinner timer fires.
///
/// # Finished state (`tb.streaming = false`)
///
/// ```text
/// 💭 Thinking  ▼  [1.4s · 340 tokens]        ← expanded
/// 💭 Thinking  ▶  [1.4s · 340 tokens]  (t)   ← collapsed, hint shown
/// ```
pub fn render_thinking_header(tb: &ThinkingBlock, spinner: &[&str]) -> Line<'static> {
    if tb.streaming {
        // Live spinner: pick the frame based on elapsed milliseconds.
        let ms = tb.start_instant.elapsed().as_millis() as usize;
        let frame = spinner[(ms / 100) % spinner.len()];
        Line::from(vec![
            Span::from("💭 Thinking  "),
            Span::from(frame.to_string()).yellow(),
            Span::from(format!(
                "  {:.1}s",
                tb.start_instant.elapsed().as_secs_f32()
            ))
            .dim(),
        ])
    } else {
        // Finished: show collapsed/expanded triangle, frozen stats, and a hint
        // when collapsed to remind the user that `t` (or a click) expands it.
        let arrow = if tb.expanded { "▼" } else { "▶" };
        let hint = if tb.expanded { "" } else { "  (t)" };
        Line::from(vec![
            Span::from(format!("💭 Thinking  {arrow}  ")),
            Span::from(format!(
                "[{:.1}s · {} tokens]",
                tb.elapsed_secs, tb.token_count
            ))
            .dim(),
            Span::from(hint).dark_gray(),
        ])
    }
}
