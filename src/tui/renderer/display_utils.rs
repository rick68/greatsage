//! Display utility functions — line flattening and word-wrapping.
//!
//! These are pure functions with no Bevy or widget-rendering dependencies; they
//! operate on slices of [`Line`] and parallel [`ClickAction`] slices so they
//! can be unit-tested in isolation.
//!
//! # Key design: parallel map propagation
//!
//! The mouse click handler needs to know which [`OutputBlock`] and
//! [`ClickAction`] corresponds to each *visual* row that the renderer paints.
//! Because word-wrapping can expand one logical line into several visual rows,
//! the mapping must survive the wrap step.
//!
//! [`hard_wrap_output_lines_with_map`] handles this by taking both the `lines`
//! slice and a parallel `map` slice, processing them together, and emitting a
//! new `(Vec<Line>, Vec<Option<…>>)` pair where every wrapped visual row
//! inherits the map entry of its origin logical line.
//!
//! [`Line`]: ratatui::text::Line
//! [`OutputBlock`]: crate::tui::core::OutputBlock
//! [`ClickAction`]: crate::tui::core::ClickAction

use {
    crate::tui::core::{ClickAction, OutputBlock, TuiMain},
    unicode_width::{UnicodeWidthChar, UnicodeWidthStr},
};

/// Builds the list of display strings for the input text box.
///
/// Prepends `prompt_prefix` to `tui.input`, then hard-wraps at `inner_width`
/// columns (respecting multi-byte / double-width Unicode characters).
///
/// Returns one `String` per visual line; the caller converts these to Ratatui
/// `Line`s when rendering.
pub fn input_display_lines(
    tui: &TuiMain,
    inner_width: usize,
    prompt_prefix: impl AsRef<str>,
) -> Vec<String> {
    let full = format!("{}{}", prompt_prefix.as_ref(), tui.input);
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;
    for c in full.chars() {
        let c_width = c.width().unwrap_or(0);
        // Start a new visual line when the next character would overflow.
        if current_width + c_width > inner_width {
            () = lines.push(current_line);
            current_line = String::new();
            current_width = 0;
        }
        () = current_line.push(c);
        current_width += c_width;
    }
    // Always push the final (possibly partial) line.
    () = lines.push(current_line);
    lines
}

/// Hard-wraps `lines` to `inner_width` columns, propagating the parallel `map`.
///
/// Each logical `lines[i]` is split into one or more visual rows.  Every new
/// visual row gets the same `map[i]` entry so the mouse handler can still
/// identify the originating block and click action after wrapping.
///
/// # Wrapping strategy
///
/// The function processes each [`Span`] character-by-character.  When adding
/// the next character would exceed `inner_width`, the current span fragment is
/// flushed and a new visual row begins.  Individual characters that are wider
/// than `inner_width` (rare in practice) are placed on their own row.
///
/// [`Span`]: ratatui::text::Span
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
                // Scan ahead to find how many characters fit on this visual row.
                let mut break_idx = span_text.len();
                let mut word_width = 0;
                for (idx, c) in span_text.char_indices() {
                    let c_width = c.width().unwrap_or(0);
                    if current_width + word_width + c_width > inner_width {
                        if word_width > 0 {
                            // Break before this character; there is space for what came before.
                            break_idx = idx;
                        } else {
                            // The very first character is already too wide — force it through.
                            break_idx = idx + c.len_utf8();
                            word_width += c_width;
                        }
                        break;
                    }
                    word_width += c_width;
                }
                // Emit the fragment that fits and continue with the remainder.
                let (head, tail) = span_text.split_at(break_idx);
                () = current_line_spans.push(Span::styled(head.to_string(), span.style));
                current_width += word_width;
                span_text = tail;
                // If the row is now full and there is still content, flush to a new visual row.
                if current_width >= inner_width && !span_text.is_empty() {
                    wrapped.push(Line::from(current_line_spans));
                    wrapped_map.push(action_info);
                    current_line_spans = Vec::new();
                    current_width = 0;
                }
            }
        }
        // Flush the trailing (possibly partial) visual row for this logical line.
        () = wrapped.push(Line::from(current_line_spans));
        () = wrapped_map.push(action_info);
    }
    (wrapped, wrapped_map)
}

/// Returns the visual column width of the input text up to the current cursor.
///
/// Uses [`UnicodeWidthStr`] so double-width CJK characters count as 2 columns.
/// This is used by the renderer to position the software cursor correctly.
pub fn display_index(tui: &TuiMain) -> usize {
    tui.input[..tui.byte_index].width()
}

/// Flattens every [`OutputBlock`] in `tui.blocks` into a single list of
/// `(Line, Option<(block_idx, ClickAction)>)` pairs.
///
/// * [`OutputBlock::Lines`] entries are appended with `map = None` (no click action).
/// * [`OutputBlock::Response`] entries are rendered via
///   [`widgets::response::render_response_lines`] and get `map = Some((block_idx, action))`.
///
/// The returned vectors are always the same length and are ready to be zipped.
///
/// [`OutputBlock`]: crate::tui::core::OutputBlock
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
                // Plain lines carry no click action; map entry is None.
                for line in lines {
                    () = all_lines.push(line.clone());
                    () = map.push(None);
                }
            }
            OutputBlock::Response(resp) => {
                // A block is highlighted (selected) when selected_block matches its index.
                let is_selected = tui.selected_block == Some(block_idx);
                let (lines, actions) =
                    crate::tui::renderer::widgets::response::render_response_lines(
                        resp,
                        is_selected,
                        spinner,
                    );
                // Each rendered line gets (block_idx, action) so the mouse handler
                // can route clicks back to the correct block and sub-element.
                for (line, action) in lines.into_iter().zip(actions) {
                    () = all_lines.push(line);
                    () = map.push(Some((block_idx, action)));
                }
            }
        }
    }
    (all_lines, map)
}
