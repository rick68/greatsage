//! Cursor animation — the two modes that share the same input box cursor.
//!
//! # Design — Blink ↔ Breathing as one lifecycle
//!
//! The cursor has two phases that mirror the Mac's own two states:
//!
//! | Phase       | Trigger                                   | Visual                        |
//! |-------------|-------------------------------------------|-------------------------------|
//! | **Blink**   | Any keypress resets the idle clock        | Classic on/off toggle         |
//! | **Breathing** | Idle for `tui.cursor_idle_sleep_secs` s | Apple-style LED pulse         |
//!
//! The transition is automatic: after `cursor_idle_sleep_secs` seconds of inactivity the
//! renderer switches the mode to `Breathing`; the first keypress switches it back to
//! `Blink` and resets [`TuiMain::last_activity`].
//!
//! # Breathing curve — Apple MacBook Sleep Indicator LED
//!
//! Introduced with the PowerBook G4 (2001), the Sleep Indicator LED on Apple portables
//! pulses with a distinctive asymmetric rhythm that Apple engineers deliberately tuned
//! to mimic the average human resting breathing rate (~12 breaths per minute, i.e. a
//! ~5-second cycle).
//!
//! The waveform is **not** a symmetric sine wave.  It uses two Gaussian half-bells
//! joined at the peak — one narrow (inhale) and one wide (exhale):
//!
//! ```text
//!         1.0 ┤        ╭╮
//!             │       ╭  ╮
//!             │      ╭    ╮
//!             │     ╭      ╮
//!             │    ╭        ╮
//!    0.05 ┤───╯               ╰──────────────╮  (5 % floor)
//!             0   0.8 s      ←── 5 s ──→
//!                peak     exhale tail + pause
//! ```
//!
//! - **Inhale (rise)**: narrow Gaussian (small σ) — LED brightens quickly.
//! - **Exhale (fall)**: wide Gaussian (large σ) — LED dims slowly.
//! - **Pause**: the long Gaussian tail naturally creates a dark "rest" interval before
//!   the next inhale; no explicit pause constant is needed.
//!
//! [`TuiMain::last_activity`]: crate::tui::core::TuiMain::last_activity

use {
    ratatui::{
        style::{Color, Style},
        text::{Line, Span},
    },
    unicode_width::{UnicodeWidthChar, UnicodeWidthStr},
};

// ── Breathing curve ────────────────────────────────────────────────────────────

/// Computes a normalised brightness value in `[0.0, 1.0]` for the current breathing `phase`.
///
/// # Mathematical form
///
/// Asymmetric (split-normal) Gaussian, normalised so `f(μ) = 1.0`:
///
/// ```text
/// f(t) = exp( −(t − μ)² / (2σ²) )
/// ```
///
/// σ switches at `t = μ` (the peak):
///
/// | Parameter | Value  | Meaning                              |
/// |-----------|--------|--------------------------------------|
/// | period    | 5.0 s  | full inhale → exhale → pause cycle   |
/// | μ         | 0.8 s  | peak centre (LED at maximum)         |
/// | σ\_rise   | 0.25 s | narrow → fast inhale                 |
/// | σ\_fall   | 1.5 s  | wide  → slow exhale + natural pause  |
pub fn breathing_brightness(phase: f32) -> f32 {
    const PERIOD: f32 = 5.0;
    const MU: f32 = 0.8;
    const SIGMA_RISE: f32 = 0.25;
    const SIGMA_FALL: f32 = 1.5;

    let t = phase * PERIOD;
    let sigma = if t <= MU { SIGMA_RISE } else { SIGMA_FALL };
    (-(t - MU).powi(2) / (2.0 * sigma.powi(2))).exp()
}

/// Returns the cursor [`Style`] for the given breathing `phase`.
///
/// # Background colour
///
/// Maps `breathing_brightness(phase)` through the ANSI 256-colour grayscale ramp
/// (indices 232 – 255), remapped to `[5 %, 100 %]` so the cursor never fully
/// disappears:
///
/// | Phase | Brightness | `Color::Indexed` |
/// |-------|-----------|------------------|
/// | tail  | 5 %       | 233 (near-black) |
/// | peak  | 100 %     | 255 (near-white) |
///
/// # Foreground colour — adaptive contrast (the bug fix)
///
/// `fg(Color::Black)` was previously used unconditionally. On a dark terminal this
/// makes the cursor character **invisible** during the dark phase (black text on a
/// near-black background). The fix uses the brightness threshold to pick a
/// contrasting foreground:
///
/// - brightness > 50 % → `fg(Color::Black)` — dark text on bright background.
/// - brightness ≤ 50 % → `fg(Color::White)` — bright text on dark background.
pub fn breathing_cursor_style(phase: f32) -> Style {
    let brightness = breathing_brightness(phase);
    // Remap [0, 1] → [0.05, 1.0]: 5 % floor keeps the cursor always visible.
    let bg_t = 0.05 + 0.95 * brightness;
    let bg = Color::Indexed(232 + (bg_t * 23.0).round() as u8);
    // Adaptive contrast: pick whichever text colour contrasts the background.
    let fg = if brightness > 0.5 {
        Color::Black
    } else {
        Color::White
    };
    Style::default().bg(bg).fg(fg)
}

// ── Input-line builder ─────────────────────────────────────────────────────────

/// Builds styled [`Line`]s for the input box in [`CursorState::Breathing`] mode.
///
/// Only the single character cell under the cursor receives a style; every other
/// character is left as a plain [`Span::raw`] so the breathing effect is strictly
/// isolated to the cursor position.
///
/// [`CursorState::Breathing`]: crate::tui::core::CursorState::Breathing
pub fn breathing_input_lines(
    lines: &[String],
    cursor_col_total: usize,
    phase: f32,
) -> Vec<Line<'static>> {
    let cursor_style = breathing_cursor_style(phase);
    let mut accumulated = 0usize;
    let mut cursor_placed = false;
    let mut result: Vec<Line<'static>> = Vec::with_capacity(lines.len());

    for (line_idx, line) in lines.iter().enumerate() {
        let line_w = line.width();

        if !cursor_placed && cursor_col_total < accumulated + line_w {
            // Cursor lands somewhere inside this line.
            let local_col = cursor_col_total - accumulated;
            let mut col = 0usize;
            let mut before_end = 0usize;
            let mut cursor_char = ' ';
            let mut after_start = line.len();

            for (byte_idx, ch) in line.char_indices() {
                if col == local_col {
                    before_end = byte_idx;
                    cursor_char = ch;
                    after_start = byte_idx + ch.len_utf8();
                    break;
                }
                col += ch.width().unwrap_or(0);
            }

            result.push(Line::from(vec![
                Span::raw(line[..before_end].to_owned()),
                Span::styled(cursor_char.to_string(), cursor_style),
                Span::raw(line[after_start..].to_owned()),
            ]));
            cursor_placed = true;
        } else if !cursor_placed && cursor_col_total == accumulated + line_w {
            // Cursor is at the very end of this line (trailing-space cursor).
            // Show trailing space only on the last line to avoid phantom rows.
            if line_idx + 1 == lines.len() {
                result.push(Line::from(vec![
                    Span::raw(line.clone()),
                    Span::styled(" ", cursor_style),
                ]));
                cursor_placed = true;
            } else {
                result.push(Line::from(line.clone()));
            }
        } else {
            result.push(Line::from(line.clone()));
        }

        accumulated += line_w;
    }

    // Cursor past all lines (empty input or cursor at the very end).
    if !cursor_placed && let Some(last) = result.last_mut() {
        last.spans.push(Span::styled(" ", cursor_style));
    }

    result
}

/// Returns the [`Style`] used for highlighted text in the output area.
pub fn selection_style() -> Style {
    Style::default().bg(Color::Indexed(240))
}
