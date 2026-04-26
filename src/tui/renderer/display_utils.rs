use {
    crate::tui::core::{ClickAction, OutputBlock, TuiMain},
    unicode_width::{UnicodeWidthChar, UnicodeWidthStr},
};

/// Calculates the text lines to be displayed in the input box.
pub fn input_display_lines(tui: &TuiMain, inner_width: usize, prompt_prefix: &str) -> Vec<String> {
    let full = format!("{}{}", prompt_prefix, tui.input);
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;
    for c in full.chars() {
        let c_width = c.width().unwrap_or(0);
        if current_width + c_width > inner_width {
            lines.push(current_line);
            current_line = String::new();
            current_width = 0;
        }
        current_line.push(c);
        current_width += c_width;
    }
    lines.push(current_line);
    lines
}

/// Performs hard wrapping on flat lines and preserves corresponding click action mappings.
pub fn hard_wrap_output_lines_with_map(
    lines: &[ratatui::text::Line<'_>],
    map: &[Option<(usize, ClickAction)>],
    inner_width: usize,
) -> (
    Vec<ratatui::text::Line<'static>>,
    Vec<Option<(usize, ClickAction)>>,
) {
    use ratatui::text::{Line, Span};
    let mut wrapped = Vec::new();
    let mut wrapped_map = Vec::new();
    for (line, &action_info) in lines.iter().zip(map) {
        let mut current_width = 0;
        let mut current_line_spans = Vec::new();
        for span in &line.spans {
            let mut span_text = span.content.as_ref();
            while !span_text.is_empty() {
                let mut break_idx = span_text.len();
                let mut word_width = 0;
                for (idx, c) in span_text.char_indices() {
                    let c_width = c.width().unwrap_or(0);
                    if current_width + word_width + c_width > inner_width {
                        if word_width > 0 {
                            break_idx = idx;
                        } else {
                            break_idx = idx + c.len_utf8();
                            word_width += c_width;
                        }
                        break;
                    }
                    word_width += c_width;
                }
                let (head, tail) = span_text.split_at(break_idx);
                current_line_spans.push(Span::styled(head.to_string(), span.style));
                current_width += word_width;
                span_text = tail;
                if current_width >= inner_width && !span_text.is_empty() {
                    wrapped.push(Line::from(current_line_spans));
                    wrapped_map.push(action_info);
                    current_line_spans = Vec::new();
                    current_width = 0;
                }
            }
        }
        wrapped.push(Line::from(current_line_spans));
        wrapped_map.push(action_info);
    }
    (wrapped, wrapped_map)
}

/// Calculates the visual offset of the current cursor in the input box.
pub fn display_index(tui: &TuiMain) -> usize {
    tui.input[..tui.byte_index].width()
}

/// Flattens all content blocks into visual lines.
pub fn rendered_flat_lines(
    tui: &TuiMain,
    spinner: &[&str],
) -> (
    Vec<ratatui::text::Line<'static>>,
    Vec<Option<(usize, ClickAction)>>,
) {
    let mut all_lines = Vec::new();
    let mut map = Vec::new();
    for (block_idx, block) in tui.blocks.iter().enumerate() {
        match block {
            OutputBlock::Lines(lines) => {
                for line in lines {
                    all_lines.push(line.clone());
                    map.push(None);
                }
            }
            OutputBlock::Response(resp) => {
                let is_selected = tui.selected_block == Some(block_idx);
                let (lines, actions) =
                    crate::tui::renderer::widgets::response::render_response_lines(
                        resp,
                        is_selected,
                        spinner,
                    );
                for (line, action) in lines.into_iter().zip(actions) {
                    all_lines.push(line);
                    map.push(Some((block_idx, action)));
                }
            }
        }
    }
    (all_lines, map)
}
