//! Renderer for a single [`ToolCallEntry`] status line.
//!
//! The line shows three possible states:
//!
//! | State | Appearance |
//! |-------|-----------|
//! | Running | `🔧 summary  ⣾  1.4s` (yellow + animated spinner) |
//! | Error   | `🔧 summary  ❌  1.4s  stderr snippet` (red) |
//! | Success | `🔧 summary  ✅  1.4s` (default + dim suffix) |
//!
//! [`ToolCallEntry`]: crate::tui::core::ToolCallEntry

use {
    crate::tui::core::ToolCallEntry,
    ratatui::{
        prelude::Stylize,
        text::{Line, Span},
    },
};

/// Renders a single tool-call status line.
///
/// `spinner` is the global spinner array; the frame is selected based on
/// `tc.start_instant.elapsed()` so it animates in sync with the spinner timer.
pub fn render_tool_call(tc: &ToolCallEntry, spinner: &[&str]) -> Line<'static> {
    let elapsed = tc.start_instant.elapsed().as_secs_f32();
    let elapsed_str = format!("{elapsed:.1}s");
    if tc.running {
        // Animate while the tool is executing.
        let ms = tc.start_instant.elapsed().as_millis() as usize;
        let frame = spinner[(ms / 100) % spinner.len()];
        Line::from(vec![
            Span::from(tc.summary.clone()).yellow(),
            Span::from(format!("  {}  {}", frame, elapsed_str))
                .yellow()
                .dim(),
        ])
    } else if tc.is_error {
        // Show the error icon and optionally a snippet from stderr.
        let mut spans = vec![
            Span::from(tc.summary.clone()).red(),
            Span::from(format!("  ❌  {elapsed_str}")).red(),
        ];
        if !tc.error_snippet.is_empty() {
            spans.push(Span::from(format!("  {}", tc.error_snippet)).red().dim());
        }
        Line::from(spans)
    } else {
        // Success: dim suffix so the elapsed time doesn't distract from content.
        Line::from(vec![
            Span::from(tc.summary.clone()),
            Span::from(format!("  ✅  {elapsed_str}")).dim(),
        ])
    }
}
