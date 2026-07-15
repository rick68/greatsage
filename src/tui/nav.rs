//! Pure scrollback navigation helpers (unit-testable, no Bevy).

use super::state::TuiState;

/// Convert `scroll_from_bottom` (0 = follow tail) to ratatui Paragraph vertical offset.
pub fn ratatui_scroll_y(line_count: usize, viewport_h: u16, scroll_from_bottom: u16) -> u16 {
    let n = line_count as u16;
    let h = viewport_h.max(1);
    if n <= h {
        return 0;
    }
    let max_from_top = n - h;
    // from_bottom=0 → max scroll (show bottom); from_bottom=max → scroll_y=0 (top)
    let from_bottom = scroll_from_bottom.min(max_from_top);
    max_from_top.saturating_sub(from_bottom)
}

/// Maximum `scroll_from_bottom` for this content/viewport.
pub fn max_scroll_from_bottom(line_count: usize, viewport_h: u16) -> u16 {
    let n = line_count as u16;
    let h = viewport_h.max(1);
    n.saturating_sub(h)
}

pub fn clamp_selected_line(selected: usize, line_count: usize) -> usize {
    if line_count == 0 {
        0
    } else {
        selected.min(line_count - 1)
    }
}

/// Adjust `scroll_from_bottom` so `selected_line` is inside the viewport.
pub fn ensure_selected_visible(state: &mut TuiState, line_count: usize) {
    if line_count == 0 {
        state.selected_line = 0;
        state.scroll_from_bottom = 0;
        return;
    }
    state.selected_line = clamp_selected_line(state.selected_line, line_count);
    let h = state.last_scrollback_height.max(1) as usize;
    let max_sfb = max_scroll_from_bottom(line_count, state.last_scrollback_height) as usize;
    state.scroll_from_bottom = state.scroll_from_bottom.min(max_sfb as u16);

    // visible absolute range: [top, top+h) where top = ratatui_scroll_y
    let top = ratatui_scroll_y(
        line_count,
        state.last_scrollback_height,
        state.scroll_from_bottom,
    ) as usize;
    let bottom = top + h; // exclusive

    if state.selected_line < top {
        // move viewport up so selection is first line
        // top' = selected → from_bottom = max_sfb - top'  (since top = max_sfb - from_bottom when n>h)
        let top_wanted = state.selected_line;
        let from_bottom = max_sfb.saturating_sub(top_wanted);
        state.scroll_from_bottom = from_bottom as u16;
    } else if state.selected_line >= bottom {
        // selection at last visible row: top' = selected - h + 1
        let top_wanted = state.selected_line + 1 - h.min(state.selected_line + 1);
        let from_bottom = max_sfb.saturating_sub(top_wanted);
        state.scroll_from_bottom = from_bottom as u16;
    }
}

pub fn move_selection_by(state: &mut TuiState, line_count: usize, delta: isize) {
    if line_count == 0 {
        state.selected_line = 0;
        return;
    }
    let cur = clamp_selected_line(state.selected_line, line_count) as isize;
    let next = (cur + delta).clamp(0, (line_count - 1) as isize) as usize;
    state.selected_line = next;
    // Leaving follow-tail when moving up from bottom
    () = ensure_selected_visible(state, line_count);
}

pub fn scroll_page(state: &mut TuiState, line_count: usize, pages_up: bool) {
    let page = state.page_size() as usize;
    let max_sfb = max_scroll_from_bottom(line_count, state.last_scrollback_height);
    if pages_up {
        state.scroll_from_bottom = state
            .scroll_from_bottom
            .saturating_add(page as u16)
            .min(max_sfb);
    } else {
        state.scroll_from_bottom = state.scroll_from_bottom.saturating_sub(page as u16);
    }
    // Move selection with page so cue stays in view
    let top = ratatui_scroll_y(
        line_count,
        state.last_scrollback_height,
        state.scroll_from_bottom,
    ) as usize;
    let h = state.last_scrollback_height.max(1) as usize;
    if line_count == 0 {
        state.selected_line = 0;
    } else {
        // keep selection inside new viewport when possible
        let sel = clamp_selected_line(state.selected_line, line_count);
        if sel < top || sel >= top + h {
            state.selected_line = if pages_up {
                top
            } else {
                top.saturating_add(h.saturating_sub(1)).min(line_count - 1)
            };
        }
    }
}

pub fn jump_home(state: &mut TuiState, line_count: usize) {
    state.selected_line = 0;
    state.scroll_from_bottom = max_scroll_from_bottom(line_count, state.last_scrollback_height);
}

pub fn jump_end(state: &mut TuiState, line_count: usize) {
    state.scroll_from_bottom = 0;
    state.selected_line = line_count.saturating_sub(1);
}

/// Jump to previous/next turn start index. `next = true` → later turn.
pub fn jump_turn(state: &mut TuiState, turn_starts: &[usize], line_count: usize, next: bool) {
    if turn_starts.is_empty() || line_count == 0 {
        return;
    }
    let sel = clamp_selected_line(state.selected_line, line_count);
    if next {
        match turn_starts.iter().find(|&&i| i > sel) {
            Some(&i) => state.selected_line = i,
            None => state.selected_line = *turn_starts.last().unwrap_or(&sel),
        }
    } else {
        match turn_starts.iter().rev().find(|&&i| i < sel) {
            Some(&i) => state.selected_line = i,
            None => state.selected_line = turn_starts[0],
        }
    }
    () = ensure_selected_visible(state, line_count);
}
