use {
    crate::tui::core::ToolCallEntry,
    ratatui::{
        prelude::Stylize,
        text::{Line, Span},
    },
};

/// Renders a tool call entry, showing status (running, error, or success) and elapsed time.
pub fn render_tool_call(tc: &ToolCallEntry, spinner: &[&str]) -> Line<'static> {
    let elapsed = tc.start_instant.elapsed().as_secs_f32();
    let elapsed_str = format!("{elapsed:.1}s");
    if tc.running {
        let ms = tc.start_instant.elapsed().as_millis() as usize;
        let frame = spinner[(ms / 100) % spinner.len()];
        Line::from(vec![
            Span::from(tc.summary.clone()).yellow(),
            Span::from(format!("  {}  {}", frame, elapsed_str))
                .yellow()
                .dim(),
        ])
    } else if tc.is_error {
        let mut spans = vec![
            Span::from(tc.summary.clone()).red(),
            Span::from(format!("  ❌  {elapsed_str}")).red(),
        ];
        if !tc.error_snippet.is_empty() {
            spans.push(Span::from(format!("  {}", tc.error_snippet)).red().dim());
        }
        Line::from(spans)
    } else {
        Line::from(vec![
            Span::from(tc.summary.clone()),
            Span::from(format!("  ✅  {elapsed_str}")).dim(),
        ])
    }
}
