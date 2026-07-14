//! Mutable TUI interaction state.

use {
    bevy::ecs::resource::Resource,
    ratatui::layout::Rect,
    std::time::{Duration, Instant},
};

/// Double-Esc clear window (Grok ~800ms).
pub const ESC_CLEAR_WINDOW: Duration = Duration::from_millis(800);

/// Max lines kept in the operator overlay panel.
const OPERATOR_PANEL_MAX: usize = 500;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TuiFocus {
    #[default]
    Prompt,
    Scrollback,
}

/// Slash autocomplete menu overlay (prompt-focused).
#[derive(Clone, Debug, Default)]
pub struct SlashMenuState {
    pub open: bool,
    pub highlight: usize,
    pub candidates: Vec<String>,
}

/// Multi-line slash/shell/help output — **separate window**, does not pollute Session ECS scrollback.
///
/// Grok-style: modal/list panel over chrome; ↑↓ navigate, Enter picks into prompt, Esc closes.
#[derive(Clone, Debug, Default)]
pub struct OperatorPanel {
    pub open: bool,
    pub title: String,
    pub lines: Vec<String>,
    /// Selected row within `lines`.
    pub selected: usize,
    /// Last drawn rect (mouse hit-test / wheel).
    pub last_rect: Rect,
}

impl OperatorPanel {
    pub fn close(&mut self) {
        self.open = false;
        () = self.lines.clear();
        self.selected = 0;
        () = self.title.clear();
        self.last_rect = Rect::default();
    }

    pub fn open_with(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.title = title.into();
        self.lines = lines;
        if self.lines.len() > OPERATOR_PANEL_MAX {
            let drop = self.lines.len() - OPERATOR_PANEL_MAX;
            self.lines.drain(0..drop);
        }
        self.selected = 0;
        self.open = !self.lines.is_empty();
    }

    pub fn append_line(&mut self, line: impl Into<String>) {
        if !self.open {
            self.open = true;
            if self.title.is_empty() {
                self.title = "output".into();
            }
        }
        self.lines.push(line.into());
        if self.lines.len() > OPERATOR_PANEL_MAX {
            let drop = self.lines.len() - OPERATOR_PANEL_MAX;
            self.lines.drain(0..drop);
            self.selected = self.selected.saturating_sub(drop);
        }
        self.selected = self.lines.len().saturating_sub(1);
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.lines.is_empty() {
            self.selected = 0;
            return;
        }
        let n = self.lines.len() as isize;
        let cur = self.selected as isize;
        let next = (cur + delta).clamp(0, n - 1);
        self.selected = next as usize;
    }

    pub fn selected_line(&self) -> Option<&str> {
        self.lines.get(self.selected).map(String::as_str)
    }
}

#[derive(Clone, Debug, Resource)]
pub struct TuiState {
    pub focus: TuiFocus,
    pub prompt: String,
    /// Byte index into `prompt`.
    pub cursor: usize,
    /// Scroll offset **from the bottom** (0 = follow tail / newest lines).
    pub scroll_from_bottom: u16,
    /// Absolute selected line index in `ScrollbackView.lines` (clamped on use).
    pub selected_line: usize,
    /// Inner scrollback row count from last layout (for page size).
    pub last_scrollback_height: u16,
    /// Last frame pane rects (for mouse hit-testing). Zero until first draw.
    pub last_scrollback_rect: Rect,
    pub last_prompt_rect: Rect,
    /// First Esc press time for double-Esc clear (prompt focused, non-empty).
    pub esc_armed_at: Option<Instant>,
    /// One-line chrome only (hints, “prompt sent”, first Esc).
    pub status_hint: Option<String>,
    /// Separate operator window (`/help`, shell, slash lists) — not scrollback.
    pub operator_panel: OperatorPanel,
    /// Slash candidate menu (Grok-style overlay; catalog from REPL completion).
    pub slash_menu: SlashMenuState,
    /// Dim ghost suffix after draft (from `inline_hint`); independent of menu open.
    pub ghost_hint: Option<String>,
}

impl Default for TuiState {
    fn default() -> Self {
        Self {
            focus: TuiFocus::Prompt,
            prompt: String::new(),
            cursor: 0,
            scroll_from_bottom: 0,
            selected_line: 0,
            last_scrollback_height: 10,
            last_scrollback_rect: Rect::default(),
            last_prompt_rect: Rect::default(),
            esc_armed_at: None,
            status_hint: None,
            operator_panel: OperatorPanel::default(),
            slash_menu: SlashMenuState::default(),
            ghost_hint: None,
        }
    }
}

/// Whether terminal cell `(column, row)` lies inside `rect` (crossterm mouse coords).
pub fn rect_contains(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && row >= rect.y
        && column < rect.x.saturating_add(rect.width)
        && row < rect.y.saturating_add(rect.height)
}

/// Extract `/command` (first token) from a help/list line for prompt fill.
pub fn draft_from_operator_line(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.starts_with('─') {
        return None;
    }
    if let Some(start) = t.find('/') {
        let rest = &t[start..];
        let label = rest.split("  ").next().unwrap_or(rest).trim();
        if let Some(cmd) = label.split_whitespace().next()
            && cmd.starts_with('/')
            && cmd.len() > 1
        {
            return Some(cmd.to_string());
        }
    }
    None
}

impl TuiState {
    /// One-line status chrome (not scrollback, not operator panel).
    pub fn set_status_hint(&mut self, line: impl Into<String>) {
        self.status_hint = Some(line.into());
    }

    /// Open/replace the operator panel with multi-line output (does **not** touch scrollback).
    pub fn open_operator_panel(
        &mut self,
        title: impl Into<String>,
        lines: impl IntoIterator<Item = String>,
    ) {
        let batch: Vec<String> = lines.into_iter().collect();
        if batch.is_empty() {
            self.operator_panel
                .open_with(title, vec!["(no output)".into()]);
        } else {
            self.operator_panel.open_with(title, batch);
        }
        () = self.clear_esc_arm();
        // Sync first selectable line into prompt if it is a slash command.
        () = self.sync_prompt_from_operator_selection();
        self.status_hint = Some("panel · ↑↓ · Enter fill · Esc close".into());
    }

    /// Stream one shell/live line into the open operator panel (or open it).
    pub fn push_operator_line(&mut self, line: impl Into<String>) {
        () = self.operator_panel.append_line(line);
        self.status_hint = Some("panel · ↑↓ · Esc close".into());
    }

    /// Copy panel selection → prompt when the line encodes a slash command.
    pub fn sync_prompt_from_operator_selection(&mut self) {
        if !self.operator_panel.open {
            return;
        }
        let Some(text) = self.operator_panel.selected_line().map(str::to_string) else {
            return;
        };
        if let Some(draft) = draft_from_operator_line(&text) {
            self.prompt = draft;
            self.cursor = self.prompt.len();
            self.slash_menu = SlashMenuState::default();
            self.ghost_hint = None;
        }
    }

    pub fn close_operator_panel(&mut self) {
        () = self.operator_panel.close();
        self.status_hint = None;
    }

    pub fn clear_prompt(&mut self) {
        () = self.prompt.clear();
        self.cursor = 0;
        self.slash_menu = SlashMenuState::default();
        self.ghost_hint = None;
    }

    pub fn clear_esc_arm(&mut self) {
        self.esc_armed_at = None;
    }

    pub fn page_size(&self) -> u16 {
        self.last_scrollback_height.saturating_sub(1).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_from_help_line() {
        assert_eq!(
            draft_from_operator_line("  /export [path]  Write session").as_deref(),
            Some("/export")
        );
    }

    #[test]
    fn panel_open_does_not_touch_scrollback_fields() {
        let mut s = TuiState::default();
        s.selected_line = 3;
        s.open_operator_panel("help", vec!["  /status  Show".into(), "  /export".into()]);
        assert!(s.operator_panel.open);
        assert_eq!(s.selected_line, 3); // scrollback selection unchanged
        assert_eq!(s.prompt, "/status"); // first selectable slash filled
    }
}
