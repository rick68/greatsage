//! Mutable TUI interaction state.

use {
    super::{
        palette::{CommandPaletteState, ShortcutsCheatsheetState},
        prompt_history::PromptHistoryBrowse,
    },
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
    /// Esc closed the menu for this exact draft; stay closed until the prompt
    /// text changes (or Tab force-opens). Prevents refresh from re-opening.
    pub dismissed_for: Option<String>,
}

/// How the operator panel handles keyboard / highlight.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OperatorPanelMode {
    /// Catalog-style (`/help`): ↑↓ highlight, Enter fills a slash into the prompt.
    #[default]
    Selectable,
    /// Read-only dump (`/tokens`, `/status`, shell): plain text; any key dismisses.
    ReadOnly,
}

/// Multi-line slash/shell/help output — **separate window**, does not pollute Session ECS scrollback.
///
/// Selectable mode: Grok-style list (↑↓ + Enter fill). ReadOnly: glance dump (any key closes).
#[derive(Clone, Debug, Default)]
pub struct OperatorPanel {
    pub open: bool,
    pub title: String,
    pub lines: Vec<String>,
    pub mode: OperatorPanelMode,
    /// Selected row within `lines` (**Selectable** only).
    pub selected: usize,
    /// First visible line index (**ReadOnly** scroll; also used as viewport anchor).
    pub scroll: usize,
    /// Last drawn rect (mouse hit-test / wheel).
    pub last_rect: Rect,
}

impl OperatorPanel {
    pub fn close(&mut self) {
        self.open = false;
        () = self.lines.clear();
        self.selected = 0;
        self.scroll = 0;
        self.mode = OperatorPanelMode::default();
        () = self.title.clear();
        self.last_rect = Rect::default();
    }

    pub fn open_with(
        &mut self,
        title: impl Into<String>,
        lines: Vec<String>,
        mode: OperatorPanelMode,
    ) {
        self.title = title.into();
        self.lines = lines;
        if self.lines.len() > OPERATOR_PANEL_MAX {
            let drop = self.lines.len() - OPERATOR_PANEL_MAX;
            self.lines.drain(0..drop);
        }
        self.mode = mode;
        self.selected = 0;
        self.scroll = 0;
        self.open = !self.lines.is_empty();
    }

    pub fn append_line(&mut self, line: impl Into<String>) {
        if !self.open {
            self.open = true;
            self.mode = OperatorPanelMode::ReadOnly;
            if self.title.is_empty() {
                self.title = "output".into();
            }
        }
        self.lines.push(line.into());
        if self.lines.len() > OPERATOR_PANEL_MAX {
            let drop = self.lines.len() - OPERATOR_PANEL_MAX;
            self.lines.drain(0..drop);
            self.selected = self.selected.saturating_sub(drop);
            self.scroll = self.scroll.saturating_sub(drop);
        }
        match self.mode {
            OperatorPanelMode::Selectable => {
                self.selected = self.lines.len().saturating_sub(1);
            }
            OperatorPanelMode::ReadOnly => {
                // Follow tail when streaming shell/live lines.
                self.scroll = self.lines.len().saturating_sub(1);
            }
        }
    }

    pub fn is_selectable(&self) -> bool {
        self.mode == OperatorPanelMode::Selectable
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

    /// Scroll viewport in **ReadOnly** mode (clamped).
    pub fn scroll_by(&mut self, delta: isize, viewport_h: usize) {
        if self.lines.is_empty() {
            self.scroll = 0;
            return;
        }
        let max_start = self.lines.len().saturating_sub(viewport_h.max(1));
        let cur = self.scroll as isize;
        let next = (cur + delta).clamp(0, max_start as isize);
        self.scroll = next as usize;
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
    /// 1-row status strip between scrollback and prompt (counts as prompt for clicks).
    pub last_status_rect: Rect,
    pub last_prompt_rect: Rect,
    /// First Esc press time for double-Esc clear (prompt focused, non-empty).
    pub esc_armed_at: Option<Instant>,
    /// One-line chrome only (hints, “prompt sent”, first Esc).
    pub status_hint: Option<String>,
    /// Separate operator window (`/help`, shell, slash lists) — not scrollback.
    pub operator_panel: OperatorPanel,
    /// Slash candidate menu (Grok-style overlay; catalog from REPL completion).
    pub slash_menu: SlashMenuState,
    /// Global command palette (`Ctrl+P` / `?`) — ephemeral; not Session ECS.
    pub command_palette: CommandPaletteState,
    /// Shortcuts cheatsheet (`Ctrl+X` / `Ctrl+.`) — ephemeral.
    pub shortcuts_cheatsheet: ShortcutsCheatsheetState,
    /// Dim ghost suffix after draft (from `inline_hint`); independent of menu open.
    pub ghost_hint: Option<String>,
    /// IME / composition preedit text (not yet committed into `prompt`).
    /// Shown after the caret; terminal IME also anchors to the hardware cursor we set.
    pub ime_preedit: String,
    /// Grok-style empty-`↑` prompt history browse (ephemeral; not Session ECS).
    pub prompt_history: PromptHistoryBrowse,
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
            last_status_rect: Rect::default(),
            last_prompt_rect: Rect::default(),
            esc_armed_at: None,
            status_hint: None,
            operator_panel: OperatorPanel::default(),
            slash_menu: SlashMenuState::default(),
            command_palette: CommandPaletteState::default(),
            shortcuts_cheatsheet: ShortcutsCheatsheetState::default(),
            ghost_hint: None,
            ime_preedit: String::new(),
            prompt_history: PromptHistoryBrowse::default(),
        }
    }
}

/// Whether terminal cell `(column, row)` lies inside `rect` (crossterm mouse coords).
pub fn rect_contains(rect: Rect, column: u16, row: u16) -> bool {
    // Empty / pre-draw rects must not steal hits (default `Rect` is 0×0 at 0,0).
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    column >= rect.x
        && row >= rect.y
        && column < rect.x.saturating_add(rect.width)
        && row < rect.y.saturating_add(rect.height)
}

/// Prompt pane **or** the 1-row status above it — operator aims here to type.
pub fn hit_prompt_or_status(prompt: Rect, status: Rect, column: u16, row: u16) -> bool {
    rect_contains(prompt, column, row) || rect_contains(status, column, row)
}

/// Close overlays that steal printable keys (panel / palette / cheatsheet).
///
/// Slash menu stays — it is tied to the draft and does not block free typing the
/// same way (chars still edit the prompt). Used when the operator clicks chrome
/// outside the operator panel so focus + typing match visual intent.
pub fn dismiss_key_stealing_overlays(state: &mut TuiState) {
    if state.operator_panel.open {
        state.close_operator_panel();
    }
    if state.command_palette.open {
        state.close_command_palette();
    }
    if state.shortcuts_cheatsheet.open {
        state.close_shortcuts_cheatsheet();
    }
    if state.prompt_history.is_open() {
        let restore = super::prompt_history::close_restore(&mut state.prompt_history);
        state.prompt = restore;
        state.cursor = state.prompt.len();
    }
}

/// Extract `/command` (first token) from a help/list line for prompt fill.
///
/// Rejects filesystem paths (`/Users/…`, `/tmp/…`) so `/tokens` / tool tables do
/// not hijack the draft when the operator navigates the OperatorPanel.
pub fn draft_from_operator_line(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.starts_with('─') {
        return None;
    }
    if let Some(start) = t.find('/') {
        let rest = &t[start..];
        let label = rest.split("  ").next().unwrap_or(rest).trim();
        if let Some(cmd) = label.split_whitespace().next()
            && is_slash_command_token(cmd)
        {
            return Some(cmd.to_string());
        }
    }
    None
}

/// Slash command tokens look like `/tokens` or `/clear!`, not absolute paths.
fn is_slash_command_token(cmd: &str) -> bool {
    let Some(name) = cmd.strip_prefix('/') else {
        return false;
    };
    if name.is_empty() {
        return false;
    }
    // Paths contain another `/` (e.g. `/Users/x`); command names do not.
    if name.contains('/') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '!')
}

impl TuiState {
    /// One-line status chrome (not scrollback, not operator panel).
    pub fn set_status_hint(&mut self, line: impl Into<String>) {
        self.status_hint = Some(line.into());
    }

    /// Open catalog-style panel (`/help`): ↑↓ select, Enter fills slash.
    pub fn open_operator_panel(
        &mut self,
        title: impl Into<String>,
        lines: impl IntoIterator<Item = String>,
    ) {
        () = self.open_operator_panel_mode(title, lines, OperatorPanelMode::Selectable);
    }

    /// Open read-only dump (`/tokens`, `/status`, auth, shell): no pick-list; any key closes.
    pub fn open_operator_panel_readonly(
        &mut self,
        title: impl Into<String>,
        lines: impl IntoIterator<Item = String>,
    ) {
        () = self.open_operator_panel_mode(title, lines, OperatorPanelMode::ReadOnly);
    }

    fn open_operator_panel_mode(
        &mut self,
        title: impl Into<String>,
        lines: impl IntoIterator<Item = String>,
        mode: OperatorPanelMode,
    ) {
        let batch: Vec<String> = lines.into_iter().collect();
        let body = if batch.is_empty() {
            vec!["(no output)".into()]
        } else {
            batch
        };
        self.operator_panel.open_with(title, body, mode);
        () = self.clear_esc_arm();
        match mode {
            OperatorPanelMode::Selectable => {
                // Sync first selectable line into prompt if it is a slash command.
                () = self.sync_prompt_from_operator_selection();
                self.status_hint = Some("panel · ↑↓:nav · Enter:fill · Esc:close".into());
            }
            OperatorPanelMode::ReadOnly => {
                // Minimal chrome — operator types next prompt immediately.
                self.status_hint = Some("any key:close".into());
            }
        }
    }

    /// Stream one shell/live line into the open operator panel (or open it).
    pub fn push_operator_line(&mut self, line: impl Into<String>) {
        () = self.operator_panel.append_line(line);
        self.status_hint = Some("any key:close".into());
    }

    /// Copy panel selection → prompt when the line encodes a slash command.
    pub fn sync_prompt_from_operator_selection(&mut self) {
        if !self.operator_panel.open || !self.operator_panel.is_selectable() {
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

    /// Open command palette (refreshes catalog rows). Closes cheatsheet if open.
    pub fn open_command_palette(&mut self) {
        self.shortcuts_cheatsheet.close();
        self.command_palette.open_fresh();
        () = self.clear_esc_arm();
        self.status_hint = Some("Ctrl+P:search · Enter:fill · Esc:close".into());
    }

    pub fn close_command_palette(&mut self) {
        self.command_palette.close();
        if self.status_hint.as_deref().is_some_and(|h| {
            h.starts_with("Ctrl+P:search")
                || h.starts_with("Ctrl+P:filter")
                || h.starts_with("palette")
        }) {
            self.status_hint = None;
        }
    }

    /// Toggle palette: open if closed, close if open.
    pub fn toggle_command_palette(&mut self) {
        if self.command_palette.open {
            self.close_command_palette();
        } else {
            self.open_command_palette();
        }
    }

    pub fn open_shortcuts_cheatsheet(&mut self) {
        self.shortcuts_cheatsheet.open_it();
        () = self.clear_esc_arm();
        self.status_hint = Some("Ctrl+X:keys · Esc:close".into());
    }

    pub fn close_shortcuts_cheatsheet(&mut self) {
        self.shortcuts_cheatsheet.close();
        if self
            .status_hint
            .as_deref()
            .is_some_and(|h| h.starts_with("Ctrl+X:keys") || h.starts_with("keys"))
        {
            self.status_hint = None;
        }
    }

    pub fn clear_prompt(&mut self) {
        () = self.prompt.clear();
        self.cursor = 0;
        self.slash_menu = SlashMenuState::default();
        self.ghost_hint = None;
        () = self.ime_preedit.clear();
        if self.prompt_history.is_open() {
            () = super::prompt_history::detach(&mut self.prompt_history);
        }
    }

    /// Set composer text and place cursor at end (history live-fill / accept).
    pub fn set_prompt_fill(&mut self, text: impl Into<String>) {
        self.prompt = text.into();
        self.cursor = self.prompt.len();
        self.slash_menu = SlashMenuState::default();
        self.ghost_hint = None;
        () = self.ime_preedit.clear();
    }

    pub fn clear_ime_preedit(&mut self) {
        self.ime_preedit.clear();
    }

    pub fn clear_esc_arm(&mut self) {
        self.esc_armed_at = None;
        // First Esc arms with a temporary hint; cancel arm (typing, other keys)
        // must restore idle key chrome, not leave "press Esc again to clear".
        if self.status_hint_is_esc_arm() {
            self.status_hint = None;
        }
    }

    fn status_hint_is_esc_arm(&self) -> bool {
        self.status_hint
            .as_deref()
            .is_some_and(|h| h.contains("Esc again") || h == "press Esc again to clear")
    }

    /// Drop a stale double-Esc arm so the arm hint cannot stick past the window.
    pub fn expire_esc_arm_if_stale(&mut self, now: Instant) {
        if let Some(armed) = self.esc_armed_at {
            if now.duration_since(armed) > ESC_CLEAR_WINDOW {
                () = self.clear_esc_arm();
            }
        } else if self.status_hint_is_esc_arm() {
            // Hint without arm timestamp (should not happen) — still restore chrome.
            self.status_hint = None;
        }
    }

    /// First half of double-Esc clear (prompt non-empty, prompt-focused).
    pub fn arm_esc_clear(&mut self, now: Instant) {
        self.esc_armed_at = Some(now);
        () = self.set_status_hint("press Esc again to clear");
    }

    pub fn page_size(&self) -> u16 {
        self.last_scrollback_height.saturating_sub(1).max(1)
    }
}
