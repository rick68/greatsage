//! Split layout: scrollback (top) + single-line status + prompt (bottom).

use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Clone, Copy, Debug)]
pub struct TuiAreas {
    pub scrollback: Rect,
    pub status: Rect,
    pub prompt: Rect,
}

/// Compute Grok-like scrollback + 1-row status + prompt.
///
/// Multi-line slash / shell output lives **inside scrollback** (operator log),
/// not in an intermediate uncontrolled pane.
pub fn split_frame(area: Rect) -> TuiAreas {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);
    TuiAreas {
        scrollback: rows[0],
        status: rows[1],
        prompt: rows[2],
    }
}
