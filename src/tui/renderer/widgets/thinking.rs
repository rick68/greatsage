use {
    crate::tui::core::ThinkingBlock,
    ratatui::{
        prelude::Stylize,
        text::{Line, Span},
    },
};

/// Renders the header for a thinking block, including spinner or elapsed time.
pub fn render_thinking_header(tb: &ThinkingBlock, spinner: &[&str]) -> Line<'static> {
    if tb.streaming {
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
