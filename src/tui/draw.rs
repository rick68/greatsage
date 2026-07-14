//! Ratatui frame draw via `bevy_ratatui::RatatuiContext`.

use {
    super::{
        layout::split_frame,
        nav::{clamp_selected_line, ratatui_scroll_y},
        scrollback::ScrollbackView,
        slash_complete::SLASH_MENU_MAX_ROWS,
        state::{TuiFocus, TuiState},
    },
    crate::repl::help_data::command_short_description,
    bevy::ecs::change_detection::{Res, ResMut},
    bevy_ratatui::RatatuiContext,
    ratatui::{
        layout::{Position, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    },
    std::cell::Cell,
};

pub const DEFAULT_STATUS_HINT: &str =
    "Tab complete `/` · Tab/click focus · wheel · PgUp/Dn · Shift+←/→ · 2xEsc · Ctrl+C leave";

/// Prompt prefix shown before the draft (`"> "`).
const PROMPT_PREFIX: &str = "> ";

/// Solid white-on-black focus (menu row / scrollback selection).
/// Prefer named `Color::White` — many terminals ignore truecolor `Rgb(255,255,255)`.
fn focus_solid() -> Style {
    Style::default().bg(Color::White).fg(Color::Black)
}

/// Style for the prompt caret cell (ANSI white bg — portable).
fn white_caret_style() -> Style {
    Style::default()
        .bg(Color::White)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

/// Paint a solid white caret into the frame buffer after widgets render.
///
/// Full-block glyphs use **foreground** in some terminals; a space + white **background**
/// is the reliable Grok-style block. Named `Color::White` avoids truecolor fallbacks to black.
fn paint_white_caret(frame: &mut ratatui::Frame, prompt_area: Rect, cols_before_caret: u16) {
    // Inner content (inside border).
    if prompt_area.width < 3 || prompt_area.height < 3 {
        return;
    }
    let inner_x = prompt_area.x.saturating_add(1);
    let inner_y = prompt_area.y.saturating_add(1);
    let max_x = prompt_area
        .x
        .saturating_add(prompt_area.width.saturating_sub(2));
    let x = inner_x.saturating_add(cols_before_caret).min(max_x);
    let buf = frame.buffer_mut();
    // One cell wide (Grok-style block caret).
    if let Some(cell) = buf.cell_mut(Position { x, y: inner_y }) {
        // Space + white bg = solid bar (█ often ignores bg / uses only fg).
        cell.set_char(' ');
        cell.set_style(white_caret_style());
    }
}

/// Truncate for a single terminal row (avoid status/prompt blowout).
fn one_line(s: &str, max_cols: u16) -> String {
    let max = max_cols.max(1) as usize;
    let mut out = String::new();
    let mut cols = 0usize;
    for ch in s.chars() {
        // Treat all as width 1 for simplicity (status is ASCII-heavy).
        if cols + 1 > max {
            break;
        }
        () = out.push(ch);
        cols += 1;
    }
    out
}

/// Format a slash menu row: `/cmd` left-aligned, short description on the right (when known).
///
/// Arg / path candidates without a registry description stay name-only.
pub fn format_slash_menu_row(candidate: &str, name_col: usize, max_cols: usize) -> Line<'static> {
    let name = candidate.to_string();
    let name_w = name.chars().count().max(name_col);
    let desc = command_short_description(candidate).unwrap_or("");
    if desc.is_empty() || max_cols <= name_w + 2 {
        return Line::from(Span::raw(one_line(&name, max_cols as u16)));
    }
    let pad = " ".repeat(name_w.saturating_sub(name.chars().count()));
    let rest = max_cols.saturating_sub(name_w + 2);
    let desc_clip = one_line(desc, rest as u16);
    Line::from(vec![
        Span::raw(format!("{name}{pad}")),
        Span::styled(
            format!("  {desc_clip}"),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ])
}

pub fn draw_system(
    mut context: ResMut<RatatuiContext>,
    mut state: ResMut<TuiState>,
    scrollback: Res<ScrollbackView>,
) -> bevy::prelude::Result {
    let focus = state.focus;
    let prompt = state.prompt.clone();
    let cursor_byte = state.cursor.min(prompt.len());
    let ghost = state.ghost_hint.clone();
    let menu_open = state.slash_menu.open;
    let menu_cands = state.slash_menu.candidates.clone();
    let menu_hi = state.slash_menu.highlight;
    let panel_open = state.operator_panel.open;
    let panel_title = state.operator_panel.title.clone();
    let panel_lines = state.operator_panel.lines.clone();
    let panel_sel = state.operator_panel.selected;
    let status_raw = state
        .status_hint
        .clone()
        .unwrap_or_else(|| DEFAULT_STATUS_HINT.to_string());
    let empty = scrollback.empty_placeholder || scrollback.lines.is_empty();
    let line_count = scrollback.line_count();
    let selected = clamp_selected_line(state.selected_line, line_count);
    let scroll_from_bottom = state.scroll_from_bottom;

    let body_lines: Vec<(String, bool)> = if empty {
        vec![("(empty session — type a prompt below)".to_string(), false)]
    } else {
        scrollback
            .lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                // Flatten accidental newlines so Paragraph line count == logical lines
                // (scroll offset is in logical lines; wrap would desync and scramble the pane).
                let text = l.text.replace(['\n', '\r'], " ");
                (text, i == selected)
            })
            .collect()
    };

    let captured_inner_h = Cell::new(state.last_scrollback_height);
    let captured_scroll = Cell::new(state.last_scrollback_rect);
    let captured_prompt = Cell::new(state.last_prompt_rect);
    let captured_panel = Cell::new(state.operator_panel.last_rect);

    context.draw(|frame| {
        let areas = split_frame(frame.area());
        captured_scroll.set(areas.scrollback);
        captured_prompt.set(areas.prompt);
        let inner_h = areas.scrollback.height.saturating_sub(2).max(1);
        captured_inner_h.set(inner_h);
        let scroll_inner_w = areas.scrollback.width.saturating_sub(2).max(1);

        let scroll_y = if empty {
            0
        } else {
            ratatui_scroll_y(line_count, inner_h, scroll_from_bottom)
        };

        let scroll_title = match focus {
            TuiFocus::Scrollback => " scrollback (focused) ",
            TuiFocus::Prompt => " scrollback ",
        };
        let body: Vec<Line> = body_lines
            .iter()
            .map(|(text, sel)| {
                let clipped = one_line(text, scroll_inner_w);
                if empty {
                    Line::from(Span::styled(
                        clipped,
                        Style::default().add_modifier(Modifier::DIM),
                    ))
                } else if *sel {
                    Line::from(Span::styled(clipped, focus_solid()))
                } else {
                    Line::from(clipped)
                }
            })
            .collect();
        // No Wrap: scroll units must match logical lines (see body_lines comment).
        let scroll_widget = Paragraph::new(body)
            .block(Block::default().borders(Borders::ALL).title(scroll_title))
            .scroll((scroll_y, 0));
        () = frame.render_widget(scroll_widget, areas.scrollback);

        let status = one_line(&status_raw, areas.status.width.max(1));
        () = frame.render_widget(
            Paragraph::new(status).style(Style::default().add_modifier(Modifier::DIM)),
            areas.status,
        );

        let prompt_title = match focus {
            TuiFocus::Prompt => " prompt (focused) ",
            TuiFocus::Scrollback => " prompt ",
        };
        let before = &prompt[..cursor_byte];
        let after = &prompt[cursor_byte..];
        // Leave a 1-cell hole for the caret; overwritten by paint_white_caret.
        let mut spans = vec![
            Span::raw(PROMPT_PREFIX.to_string()),
            Span::raw(before.to_string()),
        ];
        if focus == TuiFocus::Prompt {
            spans.push(Span::raw(" ".to_string()));
            spans.push(Span::raw(after.to_string()));
        } else {
            spans.push(Span::raw(after.to_string()));
        }
        if let Some(ref g) = ghost {
            spans.push(Span::styled(
                g.clone(),
                Style::default().add_modifier(Modifier::DIM),
            ));
        }
        let prompt_display = Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title(prompt_title));
        () = frame.render_widget(prompt_display, areas.prompt);

        if menu_open && !menu_cands.is_empty() {
            render_slash_menu(frame, areas.prompt, &menu_cands, menu_hi);
        }

        // Operator panel: floating window over scrollback (does not pollute conversation).
        if panel_open && !panel_lines.is_empty() {
            let panel_rect = operator_panel_rect(areas.scrollback, panel_lines.len());
            captured_panel.set(panel_rect);
            render_operator_panel(frame, panel_rect, &panel_title, &panel_lines, panel_sel);
        }

        // LAST: buffer-paint white caret so no later widget can cover it.
        if focus == TuiFocus::Prompt {
            let cols = (PROMPT_PREFIX.chars().count() + before.chars().count()) as u16;
            paint_white_caret(frame, areas.prompt, cols);
        }
    })?;

    state.last_scrollback_height = captured_inner_h.get();
    state.last_scrollback_rect = captured_scroll.get();
    state.last_prompt_rect = captured_prompt.get();
    state.operator_panel.last_rect = captured_panel.get();

    Ok(())
}

/// Centered floating rect inside scrollback (Grok-style small window).
fn operator_panel_rect(scrollback: Rect, line_count: usize) -> Rect {
    let max_h = scrollback.height.saturating_sub(2).max(5);
    let content_h = (line_count as u16).saturating_add(2).min(max_h).max(5);
    let max_w = scrollback.width.saturating_sub(4).max(20);
    let width = max_w.min(72).max(24);
    let x = scrollback.x + (scrollback.width.saturating_sub(width)) / 2;
    let y = scrollback.y + (scrollback.height.saturating_sub(content_h)) / 2;
    Rect {
        x,
        y,
        width,
        height: content_h,
    }
}

fn render_operator_panel(
    frame: &mut ratatui::Frame,
    area: Rect,
    title: &str,
    lines: &[String],
    selected: usize,
) {
    if area.width < 6 || area.height < 3 {
        return;
    }
    let n = lines.len();
    let hi = selected.min(n.saturating_sub(1));
    let max_vis = (area.height.saturating_sub(2) as usize).max(1);
    let start = if n <= max_vis {
        0
    } else {
        hi.saturating_sub(max_vis / 2).min(n - max_vis)
    };
    let end = (start + max_vis).min(n);
    let inner_w = area.width.saturating_sub(4).max(8) as usize;
    let items: Vec<ListItem> = lines[start..end]
        .iter()
        .map(|l| ListItem::new(one_line(l, inner_w as u16)))
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(hi.saturating_sub(start)));

    let title = format!(" {title} · ↑↓ · Enter · Esc ");
    () = frame.render_widget(Clear, area);
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_symbol("❯ ")
        .highlight_style(focus_solid().add_modifier(Modifier::BOLD));
    () = frame.render_stateful_widget(list, area, &mut list_state);
}

/// Draw candidate list above the prompt pane (capped rows).
fn render_slash_menu(
    frame: &mut ratatui::Frame,
    prompt_area: Rect,
    candidates: &[String],
    highlight: usize,
) {
    let n = candidates.len().min(SLASH_MENU_MAX_ROWS);
    if n == 0 || prompt_area.width < 4 {
        return;
    }
    // Place menu just above the prompt border, clipped to frame.
    let height = (n as u16).saturating_add(2).min(prompt_area.y.max(1));
    if height < 3 || prompt_area.y < 2 {
        return;
    }
    let y = prompt_area.y.saturating_sub(height);
    let menu_area = Rect {
        x: prompt_area.x,
        y,
        width: prompt_area.width,
        height,
    };

    let total = candidates.len();
    let hi = highlight.min(total.saturating_sub(1));
    let start = if total <= SLASH_MENU_MAX_ROWS {
        0
    } else {
        hi.saturating_sub(SLASH_MENU_MAX_ROWS / 2)
            .min(total - SLASH_MENU_MAX_ROWS)
    };
    let end = (start + SLASH_MENU_MAX_ROWS).min(total);
    let visible = &candidates[start..end];
    let name_col = visible.iter().map(|c| c.chars().count()).max().unwrap_or(0);
    // Inner width minus borders and highlight symbol (~2 cols for "▸ ").
    let inner_w = menu_area.width.saturating_sub(4).max(8) as usize;
    let items: Vec<ListItem> = visible
        .iter()
        .map(|c| ListItem::new(format_slash_menu_row(c, name_col, inner_w)))
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(hi.saturating_sub(start)));

    () = frame.render_widget(Clear, menu_area);
    // Match Grok foo2.png: `>` marker + light solid selection row.
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" slash "))
        .highlight_symbol("❯ ")
        .highlight_style(focus_solid().add_modifier(Modifier::BOLD));
    () = frame.render_stateful_widget(list, menu_area, &mut list_state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_row_includes_description_for_known_command() {
        let line = format_slash_menu_row("/provider", 12, 60);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("/provider"), "{text}");
        // Registry summary / short_description should appear.
        assert!(
            text.to_lowercase().contains("provider") && text.chars().count() > "/provider".len(),
            "expected description beside name, got: {text}"
        );
    }

    #[test]
    fn menu_row_path_candidate_name_only() {
        let line = format_slash_menu_row("./session.json", 8, 40);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("session.json"));
    }
}
