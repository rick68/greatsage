//! TUI color tokens — **GrokNight-inspired** (Grok Build CLI default).
//!
//! Palette values are adapted from [xai-org/grok-build](https://github.com/xai-org/grok-build)
//! `xai-grok-pager-render` `theme/groknight.rs` (neutral gray base + TokyoNight accents).
//!
//! **Living alignment:** re-diff against the current Grok pin / grok-build tip when
//! re-pinning (see `TODO.md` · Snapshot). This module is a **reduced** token set for
//! greatsage chrome only — not a fork of Grok's full `Theme` struct.
//!
//! Scope today: single default theme (no picker / no `config.toml` key). Multi-theme
//! is a later OpenSpec if needed.

use ratatui::style::{Color, Modifier, Style};

/// Helper for concise `Color::Rgb` definitions.
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// GrokNight-aligned colors used by `draw` (and any future TUI chrome).
///
/// Extra tokens (`bg_highlight`, `path`, …) are kept for Grok re-diff parity even when
/// the current frame path does not paint them yet.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // reserved accents for future chrome / re-diff completeness
pub struct TuiTheme {
    pub bg_base: Color,
    pub bg_highlight: Color,
    pub bg_visual: Color,
    pub bg_terminal: Color,

    pub text_primary: Color,
    pub text_secondary: Color,
    pub gray_dim: Color,
    pub gray: Color,

    pub accent_user: Color,
    pub accent_assistant: Color,
    pub accent_thinking: Color,
    pub accent_tool: Color,
    pub accent_system: Color,
    pub accent_error: Color,
    pub accent_success: Color,
    pub accent_running: Color,

    pub command: Color,
    pub path: Color,
    pub running: Color,
    pub warning: Color,
    pub fuzzy_accent: Color,

    pub selection_border: Color,
    pub prompt_border: Color,
    pub prompt_border_active: Color,
}

impl TuiTheme {
    /// Default TUI theme — GrokNight (Grok Build CLI default).
    ///
    /// RGB values match grok-build `Theme::groknight()` for the fields we use.
    pub const fn groknight() -> Self {
        // Anchors from grok-build groknight palette (2026-07-16 re-diff sample).
        const BG: Color = rgb(10, 10, 10); // #0a0a0a
        const BG_STORM: Color = rgb(20, 20, 20); // #141414
        const BG_HIGHLIGHT: Color = rgb(36, 36, 36); // #242424
        const FG: Color = rgb(225, 225, 225); // #e1e1e1
        const FG_DARK: Color = rgb(200, 200, 200); // #c8c8c8
        const COMMENT: Color = rgb(108, 108, 108); // #6c6c6c
        const DARK5: Color = rgb(120, 120, 120); // #787878
        const BLUE: Color = rgb(122, 162, 247); // #7aa2f7
        const CYAN: Color = rgb(125, 207, 255); // #7dcfff
        const GREEN: Color = rgb(158, 206, 106); // #9ece6a
        const MAGENTA: Color = rgb(187, 154, 247); // #bb9af7
        const ORANGE: Color = rgb(255, 158, 100); // #ff9e64
        const RED: Color = rgb(247, 118, 142); // #f7768e
        const YELLOW: Color = rgb(224, 175, 104); // #e0af68

        Self {
            bg_base: BG_STORM,
            bg_highlight: BG_HIGHLIGHT,
            bg_visual: rgb(54, 54, 54), // #363636
            bg_terminal: BG,

            text_primary: FG,
            text_secondary: FG_DARK,
            gray_dim: rgb(88, 88, 88), // #585858
            gray: COMMENT,

            accent_user: FG_DARK,
            accent_assistant: MAGENTA,
            accent_thinking: MAGENTA,
            accent_tool: DARK5,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: MAGENTA,

            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,
            fuzzy_accent: BLUE,

            selection_border: rgb(60, 60, 65),
            prompt_border: rgb(50, 50, 55),        // #323237
            prompt_border_active: rgb(80, 80, 88), // #505058
        }
    }

    /// Active theme for TUI frames. Today always GrokNight.
    pub fn current() -> &'static Self {
        static THEME: TuiTheme = TuiTheme::groknight();
        &THEME
    }

    /// Pane / overlay fill.
    pub fn base_style(&self) -> Style {
        Style::default().bg(self.bg_base).fg(self.text_primary)
    }

    /// Dim chrome (status labels, empty placeholder).
    pub fn dim_style(&self) -> Style {
        Style::default().bg(self.bg_base).fg(self.gray)
    }

    /// Secondary body text.
    pub fn secondary_style(&self) -> Style {
        Style::default().bg(self.bg_base).fg(self.text_secondary)
    }

    /// Selected row (scrollback / lists) — Grok `bg_visual`, not invert white.
    pub fn selection_style(&self) -> Style {
        Style::default()
            .bg(self.bg_visual)
            .fg(self.text_primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Hotkey chords in status / titles.
    pub fn hotkey_style(&self) -> Style {
        Style::default()
            .bg(self.bg_base)
            .fg(self.text_primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Palette search query (cyan running accent).
    pub fn search_query_style(&self) -> Style {
        Style::default()
            .bg(self.bg_base)
            .fg(self.running)
            .add_modifier(Modifier::BOLD)
    }

    pub fn search_placeholder_style(&self) -> Style {
        Style::default().bg(self.bg_base).fg(self.gray_dim)
    }

    /// Solid white block caret (Grok-like insert bar; portable ANSI white bg).
    pub fn caret_style(&self) -> Style {
        Style::default()
            .bg(Color::White)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    }

    pub fn ghost_style(&self) -> Style {
        Style::default().bg(self.bg_base).fg(self.gray_dim)
    }

    pub fn ime_preedit_style(&self) -> Style {
        Style::default()
            .bg(self.bg_base)
            .fg(self.warning)
            .add_modifier(Modifier::UNDERLINED)
    }

    /// Slash / command name in menus.
    pub fn command_style(&self) -> Style {
        Style::default().bg(self.bg_base).fg(self.command)
    }

    pub fn desc_style(&self) -> Style {
        Style::default().bg(self.bg_base).fg(self.gray)
    }

    /// Border for scrollback / idle prompt.
    pub fn border_idle(&self) -> Style {
        Style::default().fg(self.prompt_border).bg(self.bg_base)
    }

    /// Border when pane is focused.
    pub fn border_active(&self) -> Style {
        Style::default()
            .fg(self.prompt_border_active)
            .bg(self.bg_base)
    }

    /// Overlay (palette / panel / menu) border.
    pub fn border_overlay(&self) -> Style {
        Style::default().fg(self.selection_border).bg(self.bg_base)
    }

    /// Style a scrollback line from ECS projection text (`── turn N ──` / `[kind] …`).
    pub fn scrollback_line_style(&self, text: &str, selected: bool, empty: bool) -> Style {
        if empty {
            return self.dim_style();
        }
        if selected {
            return self.selection_style();
        }
        if text.starts_with("── turn") {
            return Style::default()
                .bg(self.bg_base)
                .fg(self.accent_assistant)
                .add_modifier(Modifier::BOLD);
        }
        // `lines_from_blocks` emits `[kind] body` — match common content_kind prefixes.
        let kind = text
            .strip_prefix('[')
            .and_then(|s| s.split_once(']'))
            .map(|(k, _)| k.to_ascii_lowercase());
        let fg = match kind.as_deref() {
            Some("user") | Some("prompt") | Some("human") => self.accent_user,
            Some("assistant") | Some("text") | Some("message") => self.accent_assistant,
            Some("thinking") | Some("reasoning") => self.accent_thinking,
            Some("tool") | Some("tool_call") | Some("tool_result") => self.accent_tool,
            Some("system") => self.accent_system,
            Some("error") => self.accent_error,
            _ => self.text_secondary,
        };
        Style::default().bg(self.bg_base).fg(fg)
    }
}
