//! Split layout: scrollback (top) + status + prompt (bottom).
//!
//! Region heights live here so draw/mouse capture share one contract
//! (`TuiLayoutSpacingContract`).

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Minimum scrollback region height (including borders).
pub const SCROLLBACK_MIN_HEIGHT: u16 = 3;

/// Default status region height (one chrome line).
pub const STATUS_HEIGHT_DEFAULT: u16 = 1;

/// Expanded status when hierarchy needs a second row.
pub const STATUS_HEIGHT_EXPANDED: u16 = 2;

/// Prompt region height (bordered draft + caret row).
pub const PROMPT_HEIGHT: u16 = 3;

/// Prefer keeping scrollback usable before expanding status to 2 rows.
pub const MIN_FRAME_HEIGHT_FOR_2ROW_STATUS: u16 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TuiAreas {
    pub scrollback: Rect,
    pub status: Rect,
    pub prompt: Rect,
}

/// Clamp requested status rows to 1 or 2 based on frame height (design D2).
pub fn clamp_status_rows(requested: u16, frame_height: u16) -> u16 {
    if requested >= 2 && frame_height >= MIN_FRAME_HEIGHT_FOR_2ROW_STATUS {
        STATUS_HEIGHT_EXPANDED
    } else {
        STATUS_HEIGHT_DEFAULT
    }
}

/// Grok-like scrollback + 1-row status + prompt (default).
#[allow(dead_code)] // public layout API; draw uses `split_frame_with_status_rows`
pub fn split_frame(area: Rect) -> TuiAreas {
    split_frame_with_status_rows(area, STATUS_HEIGHT_DEFAULT)
}

/// Scrollback + status (`status_rows` clamped 1–2) + prompt.
///
/// Multi-line slash / shell output lives **inside scrollback** (operator log),
/// not in an intermediate uncontrolled pane.
pub fn split_frame_with_status_rows(area: Rect, status_rows: u16) -> TuiAreas {
    let status_h = clamp_status_rows(status_rows, area.height);
    // On tiny frames, still allocate status + prompt when possible; scrollback
    // gets the remainder (may be zero-height — draw must not panic).
    let prompt_h = PROMPT_HEIGHT.min(area.height.saturating_sub(status_h));
    let fixed = status_h.saturating_add(prompt_h);
    let scroll_min = if area.height > fixed {
        SCROLLBACK_MIN_HEIGHT.min(area.height.saturating_sub(fixed))
    } else {
        0
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(scroll_min),
            Constraint::Length(status_h),
            Constraint::Length(prompt_h),
        ])
        .split(area);
    TuiAreas {
        scrollback: rows[0],
        status: rows[1],
        prompt: rows[2],
    }
}
