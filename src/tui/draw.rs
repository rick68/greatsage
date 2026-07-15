//! Ratatui frame draw via `bevy_ratatui::RatatuiContext`.

use {
    super::{
        layout::split_frame,
        nav::{clamp_selected_line, ratatui_scroll_y},
        palette::{CheatLine, PaletteRow, shortcuts_cheatsheet_lines},
        scrollback::ScrollbackView,
        slash_complete::SLASH_MENU_MAX_ROWS,
        state::{TuiFocus, TuiState},
        text_width::{caret_width_for_after, str_display_width, truncate_to_width},
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

/// Idle chrome: `Key:label` pairs (hotkey bright-white; label dim).
pub const DEFAULT_STATUS_HINT: &str =
    "Ctrl+P:commands · Ctrl+X:keys · /:menu · Tab:focus · 2xEsc:clear · Ctrl+C:leave";

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

/// Bright-white hotkey chords (`Ctrl+P`, `Esc`, …) — bold named white, not dim gray.
fn hotkey_style() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

/// Palette search **query** text — cyan complement to the white block caret
/// (white-on-white made typed chars and placeholder hard to read).
fn search_query_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Dim cyan placeholder after the caret cell (`type to search`).
fn search_placeholder_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::DIM)
}

/// Longest-first tokens highlighted as hotkeys in status / titles.
const HOTKEY_TOKENS: &[&str] = &[
    "Shift+←",
    "Shift+→",
    "Ctrl+P",
    "Ctrl+X",
    "Ctrl+C",
    "Ctrl+D",
    "Ctrl+.",
    "2× Esc",
    "2xEsc",
    "PgUp",
    "PgDn",
    "Enter",
    "Space",
    "Home",
    "End",
    "Tab",
    "Esc",
    "↑↓",
    "`/`",
    "/", // bare slash in `/:menu` (after longer tokens)
];

/// Split `text` into spans: known hotkey chords → bright white; rest → `base`.
fn line_with_hotkeys(text: &str, base: Style) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let mut best: Option<(usize, &'static str)> = None;
        for tok in HOTKEY_TOKENS {
            if let Some(i) = rest.find(tok) {
                match best {
                    Some((bi, bt)) if i > bi || (i == bi && tok.len() <= bt.len()) => {}
                    _ => best = Some((i, *tok)),
                }
            }
        }
        match best {
            Some((0, tok)) => {
                spans.push(Span::styled(tok.to_string(), hotkey_style()));
                rest = &rest[tok.len()..];
            }
            Some((i, _)) => {
                spans.push(Span::styled(rest[..i].to_string(), base));
                rest = &rest[i..];
            }
            None => {
                spans.push(Span::styled(rest.to_string(), base));
                break;
            }
        }
    }
    if spans.is_empty() {
        Line::from(Span::styled(String::new(), base))
    } else {
        Line::from(spans)
    }
}

/// Format a cheatsheet binding: keys bright white, description dim.
fn format_cheat_binding(keys: &str, desc: &str, key_col: usize, max_cols: usize) -> Line<'static> {
    let keys_disp = str_display_width(keys).max(key_col);
    let pad = " ".repeat(keys_disp.saturating_sub(str_display_width(keys)));
    let key_part = format!("{keys}{pad}");
    if max_cols <= keys_disp + 2 {
        return Line::from(Span::styled(
            one_line(&key_part, max_cols as u16),
            hotkey_style(),
        ));
    }
    let rest = max_cols.saturating_sub(keys_disp + 2);
    let desc_clip = one_line(desc, rest as u16);
    Line::from(vec![
        Span::styled(key_part, hotkey_style()),
        Span::styled(
            format!("  {desc_clip}"),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ])
}

/// Paint a solid white caret into the frame buffer after widgets render.
///
/// Full-block glyphs use **foreground** in some terminals; a space + white **background**
/// is the reliable Grok-style block. Named `Color::White` avoids truecolor fallbacks to black.
/// `width` is display columns (1 for ASCII insert point, 2 when covering a wide glyph).
fn paint_white_caret(
    frame: &mut ratatui::Frame,
    prompt_area: Rect,
    cols_before_caret: u16,
    width: usize,
) {
    // Inner content (inside border).
    if prompt_area.width < 3 || prompt_area.height < 3 || width == 0 {
        return;
    }
    let inner_x = prompt_area.x.saturating_add(1);
    let inner_y = prompt_area.y.saturating_add(1);
    let max_x = prompt_area
        .x
        .saturating_add(prompt_area.width.saturating_sub(2));
    let x0 = inner_x.saturating_add(cols_before_caret).min(max_x);
    let buf = frame.buffer_mut();
    for dx in 0..width {
        let cx = x0.saturating_add(dx as u16);
        if cx > max_x {
            break;
        }
        if let Some(cell) = buf.cell_mut(Position { x: cx, y: inner_y }) {
            // Space + white bg = solid bar (█ often ignores bg / uses only fg).
            cell.set_char(' ');
            cell.set_style(white_caret_style());
        }
    }
}

/// Truncate for a single terminal row using **display width** (CJK = 2 cols).
fn one_line(s: &str, max_cols: u16) -> String {
    truncate_to_width(s, max_cols.max(1) as usize)
}

/// Format a slash menu row: `/cmd` left-aligned, short description on the right (when known).
///
/// Arg / path candidates without a registry description stay name-only.
/// Column padding uses display width so wide glyphs align.
pub fn format_slash_menu_row(candidate: &str, name_col: usize, max_cols: usize) -> Line<'static> {
    let name = candidate.to_string();
    let name_disp = str_display_width(&name).max(name_col);
    let desc = command_short_description(candidate).unwrap_or("");
    if desc.is_empty() || max_cols <= name_disp + 2 {
        return Line::from(Span::raw(one_line(&name, max_cols as u16)));
    }
    let pad = " ".repeat(name_disp.saturating_sub(str_display_width(&name)));
    let rest = max_cols.saturating_sub(name_disp + 2);
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
    // Double-Esc arm is time-boxed; drop sticky "press Esc again…" if the window lapsed.
    () = state.expire_esc_arm_if_stale(std::time::Instant::now());

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
    let palette_open = state.command_palette.open;
    let palette_filter = state.command_palette.filter.clone();
    let palette_rows = state.command_palette.rows.clone();
    let palette_hi = state.command_palette.highlight;
    let cheatsheet_open = state.shortcuts_cheatsheet.open;
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
        let status_line = line_with_hotkeys(&status, Style::default().add_modifier(Modifier::DIM));
        () = frame.render_widget(Paragraph::new(status_line), areas.status);

        let prompt_title = match focus {
            TuiFocus::Prompt => " prompt (focused) ",
            TuiFocus::Scrollback => " prompt ",
        };
        let before = &prompt[..cursor_byte];
        let after = &prompt[cursor_byte..];
        let ime_preedit = state.ime_preedit.clone();
        // Display columns before caret; caret width matches glyph under insertion point.
        let caret_cols = (str_display_width(PROMPT_PREFIX) + str_display_width(before)) as u16;
        // While IME preedit is active, use a 1-col insert bar so the terminal can
        // draw composition at the hardware cursor without fighting a wide cover.
        let caret_w = if ime_preedit.is_empty() {
            caret_width_for_after(after)
        } else {
            1
        };
        // When covering a wide glyph (no preedit), omit it so the white bar replaces it.
        let after_rest: String =
            if focus == TuiFocus::Prompt && ime_preedit.is_empty() && !after.is_empty() {
                after.chars().skip(1).collect()
            } else {
                after.to_string()
            };
        let mut spans = vec![
            Span::raw(PROMPT_PREFIX.to_string()),
            Span::raw(before.to_string()),
        ];
        if focus == TuiFocus::Prompt {
            // Placeholder cells (= caret display width); painted white after.
            () = spans.push(Span::raw(" ".repeat(caret_w)));
            // IME composition (underlined) between caret and committed tail.
            if !ime_preedit.is_empty() {
                () = spans.push(Span::styled(
                    ime_preedit.clone(),
                    Style::default()
                        .add_modifier(Modifier::UNDERLINED)
                        .fg(Color::Yellow),
                ));
            }
            () = spans.push(Span::raw(after_rest));
        } else {
            () = spans.push(Span::raw(after.to_string()));
        }
        if let Some(ref g) = ghost {
            // Ghost only when not composing (IME preedit owns that slot).
            if ime_preedit.is_empty() {
                spans.push(Span::styled(
                    g.clone(),
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }
        }
        let prompt_display = Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title(prompt_title));
        () = frame.render_widget(prompt_display, areas.prompt);

        // Slash menu under global overlays (palette/cheatsheet drawn later).
        if menu_open && !menu_cands.is_empty() && ime_preedit.is_empty() && !palette_open {
            () = render_slash_menu(frame, areas.prompt, &menu_cands, menu_hi);
        }

        // Operator panel: floating window over scrollback (does not pollute conversation).
        if panel_open && !panel_lines.is_empty() {
            let panel_rect = operator_panel_rect(areas.scrollback, panel_lines.len());
            () = captured_panel.set(panel_rect);
            () = render_operator_panel(frame, panel_rect, &panel_title, &panel_lines, panel_sel);
        }

        // Shortcuts cheatsheet (below palette if both ever stacked).
        if cheatsheet_open {
            let lines = shortcuts_cheatsheet_lines();
            let rect = centered_overlay_rect(areas.scrollback, lines.len().saturating_add(1), 56);
            () = render_cheatsheet(frame, rect, &lines);
        }

        // Command palette on top of other chrome (except operator panel still
        // painted above scrollback; Esc closes panel first).
        if palette_open {
            // Open: vertically centered. While typing: same top edge (height shrinks
            // from the bottom) so `search: ` does not jump as matches drop.
            let rect = palette_overlay_rect(
                areas.scrollback,
                &palette_filter,
                palette_rows.len(),
            );
            () = render_command_palette(frame, rect, &palette_filter, &palette_rows, palette_hi);
        }

        // White caret on prompt when palette closed (palette paints its own search caret).
        if focus == TuiFocus::Prompt && !palette_open && !cheatsheet_open {
            paint_white_caret(frame, areas.prompt, caret_cols, caret_w);
            let inner_x = areas.prompt.x.saturating_add(1);
            let inner_y = areas.prompt.y.saturating_add(1);
            let max_x = areas
                .prompt
                .x
                .saturating_add(areas.prompt.width.saturating_sub(2));
            let hx = inner_x.saturating_add(caret_cols).min(max_x);
            () = frame.set_cursor_position(Position { x: hx, y: inner_y });
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
    centered_overlay_rect(scrollback, line_count, 72)
}

fn centered_overlay_rect(scrollback: Rect, content_lines: usize, prefer_width: u16) -> Rect {
    let max_h = scrollback.height.saturating_sub(2).max(5);
    let content_h = (content_lines as u16).saturating_add(2).min(max_h).max(5);
    let max_w = scrollback.width.saturating_sub(4).max(20);
    let width = max_w.min(prefer_width).max(24);
    let x = scrollback.x + (scrollback.width.saturating_sub(width)) / 2;
    let y = scrollback.y + (scrollback.height.saturating_sub(content_h)) / 2;
    Rect {
        x,
        y,
        width,
        height: content_h,
    }
}

/// Command palette overlay geometry.
///
/// - **Open / empty filter:** full list viewport, **vertically centered** (same idea as
///   operator panel).
/// - **While typing:** keep that **top** Y; height may shrink from the bottom as the
///   match list shortens, so `search: ` does not jump.
fn palette_overlay_rect(scrollback: Rect, filter: &str, filtered_count: usize) -> Rect {
    /// Inner: search row + separator + list viewport cap.
    const SEARCH_AND_SEP: u16 = 2;
    const MAX_LIST: u16 = 12;
    let max_h = scrollback.height.saturating_sub(2).max(7);
    // Open footprint used for centering (and as the pinned top when filtering).
    let open_height = (SEARCH_AND_SEP + MAX_LIST + 2).min(max_h).max(7);
    let max_w = scrollback.width.saturating_sub(4).max(20);
    let width = max_w.min(64).max(24);
    let x = scrollback.x + (scrollback.width.saturating_sub(width)) / 2;
    let y = scrollback.y + (scrollback.height.saturating_sub(open_height)) / 2;

    let height = if filter.is_empty() {
        open_height
    } else {
        // At least one list row for "(no matches)"; never taller than open box.
        let list_rows = (filtered_count as u16).clamp(1, MAX_LIST);
        (SEARCH_AND_SEP + list_rows + 2)
            .min(open_height)
            .max(5)
    };

    Rect {
        x,
        y,
        width,
        height,
    }
}

fn format_palette_row(row: &PaletteRow, name_col: usize, max_cols: usize) -> Line<'static> {
    let name = row.label().to_string();
    let name_disp = str_display_width(&name).max(name_col);
    let desc = row.description();
    if desc.is_empty() || max_cols <= name_disp + 2 {
        return Line::from(Span::raw(one_line(&name, max_cols as u16)));
    }
    let pad = " ".repeat(name_disp.saturating_sub(str_display_width(&name)));
    let rest = max_cols.saturating_sub(name_disp + 2);
    let desc_clip = one_line(desc, rest as u16);
    Line::from(vec![
        Span::raw(format!("{name}{pad}")),
        Span::styled(
            format!("  {desc_clip}"),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ])
}

/// In-palette search field prefix (leading space so label is not flush against the border).
const PALETTE_SEARCH_LABEL: &str = " search: ";

fn render_command_palette(
    frame: &mut ratatui::Frame,
    area: Rect,
    filter: &str,
    rows: &[PaletteRow],
    highlight: usize,
) {
    if area.width < 8 || area.height < 5 {
        return;
    }
    () = frame.render_widget(Clear, area);
    let title = line_with_hotkeys(" Ctrl+P:commands · Esc:close ", Style::default());
    // Outer block: search row (with caret) + result list.
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    () = frame.render_widget(block, area);
    if inner.height < 3 {
        return;
    }
    // Search field: one row + bottom border line so it reads as a real input.
    let search_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let sep_area = Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: 1,
    };
    let list_area = Rect {
        x: inner.x,
        y: inner.y.saturating_add(2),
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };

    // Layout: `search: ` + query(cyan) + [caret cell] + optional dim placeholder.
    // Query uses cyan (complement to white caret) so invert-white never masks text.
    let label = PALETTE_SEARCH_LABEL;
    let label_w = str_display_width(label);
    // Reserve 1 col for caret; leave room for a short placeholder when empty.
    let max_query_cols = (search_area.width as usize)
        .saturating_sub(label_w + 1 + 14)
        .max(1);
    let query_display = if filter.is_empty() {
        None
    } else {
        Some(one_line(filter, max_query_cols as u16))
    };
    let query_w = query_display
        .as_ref()
        .map(|q| str_display_width(q))
        .unwrap_or(0);
    let mut search_spans = vec![Span::styled(label.to_string(), hotkey_style())];
    if let Some(ref q) = query_display {
        search_spans.push(Span::styled(q.clone(), search_query_style()));
    }
    // Dedicated cell for the white block caret (must not share a glyph with text).
    search_spans.push(Span::raw(" "));
    if query_display.is_none() {
        search_spans.push(Span::styled(
            "type to search".to_string(),
            search_placeholder_style(),
        ));
    }
    () = frame.render_widget(Paragraph::new(Line::from(search_spans)), search_area);

    // Separator under search field.
    let sep = "─".repeat(search_area.width.max(1) as usize);
    () = frame.render_widget(
        Paragraph::new(one_line(&sep, search_area.width.max(1)))
            .style(Style::default().add_modifier(Modifier::DIM)),
        sep_area,
    );

    // White caret only on the reserved cell (after label + query).
    let caret_cols = (label_w + query_w) as u16;
    paint_white_caret_at(frame, search_area.x, search_area.y, caret_cols, 1, search_area);
    let hx = search_area
        .x
        .saturating_add(caret_cols)
        .min(search_area.x.saturating_add(search_area.width.saturating_sub(1)));
    frame.set_cursor_position(Position {
        x: hx,
        y: search_area.y,
    });

    if rows.is_empty() {
        () = frame.render_widget(
            Paragraph::new("(no matches)").style(Style::default().add_modifier(Modifier::DIM)),
            list_area,
        );
        return;
    }
    let n = rows.len();
    let hi = highlight.min(n.saturating_sub(1));
    let max_vis = (list_area.height as usize).max(1);
    let start = if n <= max_vis {
        0
    } else {
        hi.saturating_sub(max_vis / 2).min(n - max_vis)
    };
    let end = (start + max_vis).min(n);
    let visible = &rows[start..end];
    let name_col = visible
        .iter()
        .map(|r| str_display_width(r.label()))
        .max()
        .unwrap_or(0);
    let inner_w = list_area.width.saturating_sub(2).max(8) as usize;
    let items: Vec<ListItem> = visible
        .iter()
        .map(|r| ListItem::new(format_palette_row(r, name_col, inner_w)))
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(hi.saturating_sub(start)));
    let list = List::new(items)
        .highlight_symbol("❯ ")
        .highlight_style(focus_solid().add_modifier(Modifier::BOLD));
    () = frame.render_stateful_widget(list, list_area, &mut list_state);
}

/// Paint white caret at absolute `(x + cols, y)` clipped to `clip` area.
fn paint_white_caret_at(
    frame: &mut ratatui::Frame,
    base_x: u16,
    y: u16,
    cols_before: u16,
    width: usize,
    clip: Rect,
) {
    if width == 0 || clip.width == 0 {
        return;
    }
    let max_x = clip.x.saturating_add(clip.width.saturating_sub(1));
    let x0 = base_x.saturating_add(cols_before).min(max_x);
    let buf = frame.buffer_mut();
    for dx in 0..width {
        let cx = x0.saturating_add(dx as u16);
        if cx > max_x || cx < clip.x || y < clip.y || y >= clip.y.saturating_add(clip.height) {
            break;
        }
        if let Some(cell) = buf.cell_mut(Position { x: cx, y }) {
            cell.set_char(' ');
            cell.set_style(white_caret_style());
        }
    }
}

fn render_cheatsheet(frame: &mut ratatui::Frame, area: Rect, lines: &[CheatLine]) {
    if area.width < 6 || area.height < 3 {
        return;
    }
    let n = lines.len();
    let max_vis = (area.height.saturating_sub(2) as usize).max(1);
    let end = max_vis.min(n);
    let visible = &lines[..end];
    let key_col = visible
        .iter()
        .filter_map(|l| match l {
            CheatLine::Binding { keys, .. } => Some(str_display_width(keys)),
            CheatLine::Header(_) => None,
        })
        .max()
        .unwrap_or(12)
        .min(22);
    let inner_w = area.width.saturating_sub(4).max(8) as usize;
    let items: Vec<ListItem> = visible
        .iter()
        .map(|line| match line {
            CheatLine::Header(h) => ListItem::new(Line::from(Span::styled(
                one_line(h, inner_w as u16),
                Style::default().add_modifier(Modifier::DIM),
            ))),
            CheatLine::Binding { keys, desc } => {
                ListItem::new(format_cheat_binding(keys, desc, key_col, inner_w))
            }
        })
        .collect();
    () = frame.render_widget(Clear, area);
    let title = line_with_hotkeys(" Ctrl+X:keys · Esc:close ", Style::default());
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));
    () = frame.render_widget(list, area);
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
    () = list_state.select(Some(hi.saturating_sub(start)));

    let title_line = line_with_hotkeys(
        &format!(" {title} · ↑↓:nav · Enter:fill · Esc:close "),
        Style::default(),
    );
    () = frame.render_widget(Clear, area);
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title_line))
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
    let name_col = visible
        .iter()
        .map(|c| str_display_width(c))
        .max()
        .unwrap_or(0);
    // Inner width minus borders and highlight symbol (~2 cols for "▸ ").
    let inner_w = menu_area.width.saturating_sub(4).max(8) as usize;
    let items: Vec<ListItem> = visible
        .iter()
        .map(|c| ListItem::new(format_slash_menu_row(c, name_col, inner_w)))
        .collect();
    let mut list_state = ListState::default();
    () = list_state.select(Some(hi.saturating_sub(start)));

    () = frame.render_widget(Clear, menu_area);
    // Grok slash menu: marker + solid selection row.
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" slash "))
        .highlight_symbol("❯ ")
        .highlight_style(focus_solid().add_modifier(Modifier::BOLD));
    () = frame.render_stateful_widget(list, menu_area, &mut list_state);
}
