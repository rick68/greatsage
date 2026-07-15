//! Command palette: shared `help_data` catalog + small UI-only action set.
//!
//! Filter is case-insensitive substring on name/label and description.
//! Accepting a slash row fills the prompt only — does **not** dispatch.

use crate::repl::help_data::{KNOWN_COMMANDS, command_short_description};

/// Stable ids for UI-only palette rows (not slash commands).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaletteUiAction {
    /// Open the shortcuts cheatsheet overlay.
    OpenShortcuts,
    /// Clear the prompt draft (same effect as double-Esc clear).
    ClearPrompt,
}

/// One palette list row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaletteRow {
    Slash {
        name: &'static str,
        description: &'static str,
    },
    Ui {
        action: PaletteUiAction,
        label: &'static str,
        description: &'static str,
    },
}

impl PaletteRow {
    pub fn label(&self) -> &str {
        match self {
            Self::Slash { name, .. } => name,
            Self::Ui { label, .. } => label,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Slash { description, .. } => description,
            Self::Ui { description, .. } => description,
        }
    }

    /// Primary search haystack (label + description).
    pub fn search_text(&self) -> String {
        format!("{} {}", self.label(), self.description())
    }
}

/// Build full unfiltered row list: UI-only first, then `KNOWN_COMMANDS`.
pub fn all_palette_rows() -> Vec<PaletteRow> {
    let mut rows = vec![
        PaletteRow::Ui {
            action: PaletteUiAction::OpenShortcuts,
            label: "keyboard shortcuts",
            description: "Show simple-mode key bindings (Ctrl+X:keys)",
        },
        PaletteRow::Ui {
            action: PaletteUiAction::ClearPrompt,
            label: "clear prompt",
            description: "Clear the draft prompt",
        },
    ];
    for cmd in KNOWN_COMMANDS {
        let description = command_short_description(cmd.name).unwrap_or(cmd.summary);
        () =  rows.push(PaletteRow::Slash {
            name: cmd.name,
            description,
        });
    }
    rows
}

/// Case-insensitive substring filter on label + description.
pub fn filter_palette_rows(rows: &[PaletteRow], filter: &str) -> Vec<PaletteRow> {
    let q = filter.trim();
    if q.is_empty() {
        return rows.to_vec();
    }
    let q_lower = q.to_ascii_lowercase();
    rows.iter()
        .filter(|r| r.search_text().to_ascii_lowercase().contains(&q_lower))
        .cloned()
        .collect()
}

/// Mutable palette chrome on `TuiState`.
#[derive(Clone, Debug, Default)]
pub struct CommandPaletteState {
    pub open: bool,
    /// Filter typed while palette is open (not the prompt draft).
    pub filter: String,
    pub highlight: usize,
    /// Last filtered rows (refreshed on open / filter change).
    pub rows: Vec<PaletteRow>,
}

impl CommandPaletteState {
    pub fn close(&mut self) {
        self.open = false;
        self.filter.clear();
        self.highlight = 0;
        self.rows.clear();
    }

    pub fn open_fresh(&mut self) {
        self.open = true;
        self.filter.clear();
        self.highlight = 0;
        self.rows = all_palette_rows();
    }

    pub fn refresh_filter(&mut self) {
        let all = all_palette_rows();
        self.rows = filter_palette_rows(&all, &self.filter);
        if self.rows.is_empty() {
            self.highlight = 0;
        } else {
            self.highlight = self.highlight.min(self.rows.len() - 1);
        }
    }

    pub fn move_highlight(&mut self, delta: isize) {
        if self.rows.is_empty() {
            self.highlight = 0;
            return;
        }
        let n = self.rows.len() as isize;
        let cur = self.highlight as isize;
        self.highlight = (cur + delta).clamp(0, n - 1) as usize;
    }

    pub fn selected(&self) -> Option<&PaletteRow> {
        self.rows.get(self.highlight)
    }

    pub fn insert_filter_char(&mut self, c: char) {
        self.filter.push(c);
        self.refresh_filter();
    }

    pub fn filter_backspace(&mut self) {
        let _ = self.filter.pop();
        self.refresh_filter();
    }
}

/// Result of accepting a palette row (caller applies to `TuiState`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaletteAccept {
    /// Fill prompt with this slash token; do not dispatch.
    FillSlash(String),
    /// Run UI-only action.
    Ui(PaletteUiAction),
}

pub fn accept_selected(palette: &CommandPaletteState) -> Option<PaletteAccept> {
    match palette.selected()? {
        PaletteRow::Slash { name, .. } => Some(PaletteAccept::FillSlash((*name).to_string())),
        PaletteRow::Ui { action, .. } => Some(PaletteAccept::Ui(*action)),
    }
}

/// One line in the shortcuts cheatsheet (keys render bright-white in draw).
#[derive(Clone, Copy, Debug)]
pub enum CheatLine {
    /// Section header (dim, not a hotkey).
    Header(&'static str),
    /// Left: key chord(s); right: description.
    Binding {
        keys: &'static str,
        desc: &'static str,
    },
}

/// Static cheatsheet rows for simple-mode bindings greatsage implements.
pub fn shortcuts_cheatsheet_lines() -> Vec<CheatLine> {
    vec![
        CheatLine::Header("── Focus ──"),
        CheatLine::Binding {
            keys: "Tab",
            desc: "Toggle prompt ↔ scrollback",
        },
        CheatLine::Binding {
            keys: "Space / printable",
            desc: "Focus prompt (from scrollback)",
        },
        CheatLine::Header("── Scrollback ──"),
        CheatLine::Binding {
            keys: "↑↓ / wheel",
            desc: "Move selection",
        },
        CheatLine::Binding {
            keys: "PgUp / PgDn",
            desc: "Page scroll",
        },
        CheatLine::Binding {
            keys: "Shift+← / Shift+→",
            desc: "Previous / next turn",
        },
        CheatLine::Binding {
            keys: "Home / End",
            desc: "Top / bottom",
        },
        CheatLine::Header("── Prompt ──"),
        CheatLine::Binding {
            keys: "/",
            desc: "Slash menu + ghost (shared catalog)",
        },
        CheatLine::Binding {
            keys: "Enter",
            desc: "Send prompt / run slash",
        },
        CheatLine::Binding {
            keys: "2× Esc",
            desc: "Clear draft (prompt focused)",
        },
        CheatLine::Header("── Overlays ──"),
        CheatLine::Binding {
            keys: "Ctrl+P",
            desc: "Command palette (toggle)",
        },
        CheatLine::Binding {
            keys: "?",
            desc: "Palette (empty draft) / insert ?",
        },
        CheatLine::Binding {
            keys: "Ctrl+X / Ctrl+.",
            desc: "Keyboard shortcuts",
        },
        CheatLine::Binding {
            keys: "Esc",
            desc: "Close one overlay layer",
        },
        CheatLine::Header("── Agent ──"),
        CheatLine::Binding {
            keys: "Ctrl+C",
            desc: "Cancel turn / arm leave; 2× leave",
        },
        CheatLine::Binding {
            keys: "Ctrl+D",
            desc: "Leave (idle)",
        },
    ]
}

#[derive(Clone, Debug, Default)]
pub struct ShortcutsCheatsheetState {
    pub open: bool,
}

impl ShortcutsCheatsheetState {
    pub fn open_it(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}
