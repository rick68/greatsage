//! TUI color tokens — multi-theme catalog (Grok Build CLI–aligned).
//!
//! Built-in palettes are a **reduced** hand-port of [xai-org/grok-build](https://github.com/xai-org/grok-build)
//! theme modules (`xai-grok-pager-render` / guide `06-theming`), not a crate dependency.
//!
//! **Living alignment:** re-diff RGB anchors against the current Grok pin when re-pinning
//! (see `TODO.md` · Snapshot / `docs/alignment/grok-build-report.md`).
//!
//! Catalog: `groknight` (default) · `grokday` · `tokyonight` · `rosepine` · `oscura`.
//! Runtime selection + picker + config `theme` key — not Session ECS / BRP.

use ratatui::style::{Color, Modifier, Style};

/// Helper for concise `Color::Rgb` definitions.
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Built-in theme catalog ids (canonical form used in config).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum ThemeId {
    #[default]
    GrokNight,
    GrokDay,
    TokyoNight,
    RosePine,
    Oscura,
}

/// Catalog order for cycle (`/theme` bare) and picker list.
pub const CATALOG: &[ThemeId] = &[
    ThemeId::GrokNight,
    ThemeId::GrokDay,
    ThemeId::TokyoNight,
    ThemeId::RosePine,
    ThemeId::Oscura,
];

impl ThemeId {
    /// Canonical config / slash id (lowercase, no hyphens).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GrokNight => "groknight",
            Self::GrokDay => "grokday",
            Self::TokyoNight => "tokyonight",
            Self::RosePine => "rosepine",
            Self::Oscura => "oscura",
        }
    }

    /// Operator-facing label for the picker list.
    pub const fn label(self) -> &'static str {
        match self {
            Self::GrokNight => "GrokNight",
            Self::GrokDay => "GrokDay",
            Self::TokyoNight => "TokyoNight",
            Self::RosePine => "RosePineMoon",
            Self::Oscura => "OscuraMidnight",
        }
    }

    /// Resolve a config / slash name (case-insensitive) with Grok-style aliases.
    ///
    /// Unknown / empty / unsupported (`auto`, `system`) → [`ThemeId::GrokNight`].
    pub fn resolve(name: &str) -> Self {
        let n = name.trim().to_ascii_lowercase().replace('_', "-");
        if n.is_empty() {
            return Self::GrokNight;
        }
        // Reject auto/system for v1 (no system appearance polling).
        if n == "auto" || n == "system" {
            return Self::GrokNight;
        }
        match n.as_str() {
            "groknight" | "grok-night" | "dark" => Self::GrokNight,
            "grokday" | "grok-day" | "light" | "day" => Self::GrokDay,
            "tokyonight" | "tokyo-night" | "tokyo" => Self::TokyoNight,
            "rosepine" | "rose-pine" | "rosepine-moon" | "rose-pine-moon" | "rose" => {
                Self::RosePine
            }
            "oscura" | "oscura-midnight" | "midnight" => Self::Oscura,
            _ => Self::GrokNight,
        }
    }

    /// Whether `name` is a known id or alias (not unknown fallback).
    pub fn is_known_name(name: &str) -> bool {
        let n = name.trim().to_ascii_lowercase().replace('_', "-");
        if n.is_empty() || n == "auto" || n == "system" {
            return false;
        }
        matches!(
            n.as_str(),
            "groknight"
                | "grok-night"
                | "dark"
                | "grokday"
                | "grok-day"
                | "light"
                | "day"
                | "tokyonight"
                | "tokyo-night"
                | "tokyo"
                | "rosepine"
                | "rose-pine"
                | "rosepine-moon"
                | "rose-pine-moon"
                | "rose"
                | "oscura"
                | "oscura-midnight"
                | "midnight"
        )
    }

    /// Next catalog entry after `self` (wraps).
    pub fn next(self) -> Self {
        let idx = CATALOG.iter().position(|t| *t == self).unwrap_or(0);
        CATALOG[(idx + 1) % CATALOG.len()]
    }

    /// Index in [`CATALOG`] (for picker highlight).
    pub fn catalog_index(self) -> usize {
        CATALOG.iter().position(|t| *t == self).unwrap_or(0)
    }
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
    /// Resolve tokens for a catalog id.
    #[allow(dead_code)] // available for tests / future non-static paths
    pub const fn for_id(id: ThemeId) -> Self {
        match id {
            ThemeId::GrokNight => Self::groknight(),
            ThemeId::GrokDay => Self::grokday(),
            ThemeId::TokyoNight => Self::tokyonight(),
            ThemeId::RosePine => Self::rosepine(),
            ThemeId::Oscura => Self::oscura(),
        }
    }

    /// Static reference for draw (themes are pure const data).
    pub fn get(id: ThemeId) -> &'static Self {
        match id {
            ThemeId::GrokNight => {
                static T: TuiTheme = TuiTheme::groknight();
                &T
            }
            ThemeId::GrokDay => {
                static T: TuiTheme = TuiTheme::grokday();
                &T
            }
            ThemeId::TokyoNight => {
                static T: TuiTheme = TuiTheme::tokyonight();
                &T
            }
            ThemeId::RosePine => {
                static T: TuiTheme = TuiTheme::rosepine();
                &T
            }
            ThemeId::Oscura => {
                static T: TuiTheme = TuiTheme::oscura();
                &T
            }
        }
    }

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

    /// Light theme for bright terminal backgrounds (Grok GrokDay family).
    pub const fn grokday() -> Self {
        const BG: Color = rgb(250, 250, 250); // #fafafa
        const BG_BASE: Color = rgb(242, 242, 242); // #f2f2f2
        const BG_HIGHLIGHT: Color = rgb(228, 228, 228); // #e4e4e4
        const BG_VISUAL: Color = rgb(210, 210, 218); // #d2d2da
        const FG: Color = rgb(36, 40, 59); // #24283b
        const FG_SEC: Color = rgb(65, 72, 104); // #414868
        const COMMENT: Color = rgb(120, 124, 140); // #787c8c
        const BLUE: Color = rgb(47, 108, 200);
        const CYAN: Color = rgb(15, 140, 180);
        const GREEN: Color = rgb(72, 140, 60);
        const MAGENTA: Color = rgb(140, 80, 200);
        const ORANGE: Color = rgb(200, 110, 40);
        const RED: Color = rgb(200, 60, 80);
        const YELLOW: Color = rgb(160, 120, 20);

        Self {
            bg_base: BG_BASE,
            bg_highlight: BG_HIGHLIGHT,
            bg_visual: BG_VISUAL,
            bg_terminal: BG,
            text_primary: FG,
            text_secondary: FG_SEC,
            gray_dim: rgb(150, 152, 160),
            gray: COMMENT,
            accent_user: FG_SEC,
            accent_assistant: MAGENTA,
            accent_thinking: MAGENTA,
            accent_tool: COMMENT,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: MAGENTA,
            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,
            fuzzy_accent: BLUE,
            selection_border: rgb(160, 160, 170),
            prompt_border: rgb(180, 180, 190),
            prompt_border_active: rgb(100, 100, 120),
        }
    }

    /// Tokyo Night — blue-tinted dark (truecolor-friendly).
    pub const fn tokyonight() -> Self {
        const BG: Color = rgb(26, 27, 38); // #1a1b26
        const BG_BASE: Color = rgb(36, 40, 59); // #24283b
        const BG_HIGHLIGHT: Color = rgb(41, 46, 66); // #292e42
        const BG_VISUAL: Color = rgb(54, 59, 82);
        const FG: Color = rgb(192, 202, 245); // #c0caf5
        const FG_DARK: Color = rgb(169, 177, 214); // #a9b1d6
        const COMMENT: Color = rgb(86, 95, 137); // #565f89
        const BLUE: Color = rgb(122, 162, 247);
        const CYAN: Color = rgb(125, 207, 255);
        const GREEN: Color = rgb(158, 206, 106);
        const MAGENTA: Color = rgb(187, 154, 247);
        const ORANGE: Color = rgb(255, 158, 100);
        const RED: Color = rgb(247, 118, 142);
        const YELLOW: Color = rgb(224, 175, 104);

        Self {
            bg_base: BG_BASE,
            bg_highlight: BG_HIGHLIGHT,
            bg_visual: BG_VISUAL,
            bg_terminal: BG,
            text_primary: FG,
            text_secondary: FG_DARK,
            gray_dim: rgb(65, 72, 104),
            gray: COMMENT,
            accent_user: FG_DARK,
            accent_assistant: MAGENTA,
            accent_thinking: MAGENTA,
            accent_tool: COMMENT,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: CYAN,
            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,
            fuzzy_accent: BLUE,
            selection_border: rgb(65, 72, 104),
            prompt_border: rgb(59, 66, 97),
            prompt_border_active: rgb(122, 162, 247),
        }
    }

    /// Rosé Pine Moon family — muted dark + mauve accents.
    pub const fn rosepine() -> Self {
        const BG: Color = rgb(35, 33, 54); // #232136
        const BG_BASE: Color = rgb(42, 39, 63); // #2a273f
        const BG_HIGHLIGHT: Color = rgb(57, 53, 82); // #393552
        const BG_VISUAL: Color = rgb(68, 65, 90);
        const FG: Color = rgb(224, 222, 244); // #e0def4
        const FG_SEC: Color = rgb(144, 140, 170); // #908caa
        const MUTED: Color = rgb(110, 106, 134); // #6e6a86
        const IRIS: Color = rgb(196, 167, 231); // #c4a7e7
        const FOAM: Color = rgb(156, 207, 216); // #9ccfd8
        const PINE: Color = rgb(62, 143, 176); // #3e8fb0
        const GOLD: Color = rgb(246, 193, 119); // #f6c177
        const LOVE: Color = rgb(235, 111, 146); // #eb6f92
        const ROSE: Color = rgb(234, 154, 151); // #ea9a97

        Self {
            bg_base: BG_BASE,
            bg_highlight: BG_HIGHLIGHT,
            bg_visual: BG_VISUAL,
            bg_terminal: BG,
            text_primary: FG,
            text_secondary: FG_SEC,
            gray_dim: rgb(86, 82, 110),
            gray: MUTED,
            accent_user: FG_SEC,
            accent_assistant: IRIS,
            accent_thinking: IRIS,
            accent_tool: MUTED,
            accent_system: PINE,
            accent_error: LOVE,
            accent_success: FOAM,
            accent_running: IRIS,
            command: GOLD,
            path: ROSE,
            running: FOAM,
            warning: GOLD,
            fuzzy_accent: IRIS,
            selection_border: rgb(86, 82, 110),
            prompt_border: rgb(68, 65, 90),
            prompt_border_active: IRIS,
        }
    }

    /// Oscura Midnight — deep dark base with purple accents.
    pub const fn oscura() -> Self {
        const BG: Color = rgb(12, 10, 18);
        const BG_BASE: Color = rgb(18, 16, 28);
        const BG_HIGHLIGHT: Color = rgb(32, 28, 48);
        const BG_VISUAL: Color = rgb(48, 40, 72);
        const FG: Color = rgb(230, 225, 240);
        const FG_SEC: Color = rgb(180, 170, 200);
        const DIM: Color = rgb(100, 90, 120);
        const PURPLE: Color = rgb(180, 140, 255);
        const CYAN: Color = rgb(120, 200, 220);
        const GREEN: Color = rgb(140, 200, 140);
        const ORANGE: Color = rgb(240, 170, 100);
        const RED: Color = rgb(240, 100, 130);
        const YELLOW: Color = rgb(230, 200, 120);

        Self {
            bg_base: BG_BASE,
            bg_highlight: BG_HIGHLIGHT,
            bg_visual: BG_VISUAL,
            bg_terminal: BG,
            text_primary: FG,
            text_secondary: FG_SEC,
            gray_dim: rgb(70, 65, 90),
            gray: DIM,
            accent_user: FG_SEC,
            accent_assistant: PURPLE,
            accent_thinking: PURPLE,
            accent_tool: DIM,
            accent_system: CYAN,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: PURPLE,
            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,
            fuzzy_accent: PURPLE,
            selection_border: rgb(70, 60, 100),
            prompt_border: rgb(50, 45, 70),
            prompt_border_active: PURPLE,
        }
    }

    /// Pane / overlay fill.
    pub fn base_style(&self) -> Style {
        Style::default().bg(self.bg_base).fg(self.text_primary)
    }

    /// Dim chrome (status labels, empty placeholder).
    pub fn dim_style(&self) -> Style {
        Style::default().bg(self.bg_base).fg(self.gray)
    }

    /// Empty-session welcome title (product identity).
    pub fn welcome_title_style(&self) -> Style {
        Style::default()
            .bg(self.bg_base)
            .fg(self.text_primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Empty-session welcome body / key-hint lines.
    pub fn welcome_body_style(&self) -> Style {
        self.dim_style()
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
