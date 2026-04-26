//! Rendering pipeline — converts [`TuiMain`] state into Ratatui frames.
//!
//! # Frame lifecycle (every tick where `RenderNeeded` is `true`)
//!
//! 1. [`draw_scene_system`] ticks the cursor-blink and spinner timers.
//! 2. Calls [`render_tui`], which:
//!    a. Computes input-box line count to determine layout.
//!    b. Flattens all output blocks into a `Vec<Line>` + parallel `line_map` via [`display_utils::rendered_flat_lines`].
//!    c. Word-wraps both to the terminal width via [`display_utils::hard_wrap_output_lines_with_map`].
//!    d. Stores the wrapped `line_map` back into [`TuiMain`] for the mouse handler.
//!    e. Renders Output / Status / Input panels with focus-based border colors.
//!    f. Positions the software cursor inside the Input panel.
//!
//! [`TuiMain`]: crate::tui::core::TuiMain

pub mod display_utils;
pub mod widgets;

use {
    crate::{
        agents::CodingAgentTotalTokenUsage,
        tui::{
            core::{
                COLOR_BORDER_FOCUSED, COLOR_BORDER_UNFOCUSED, CURSOR_BLINK_INTERVAL_MS,
                CursorStyle, PROMPT_PREFIX, SPINNER, TuiMain, TuiMainFocus,
            },
            events::RenderNeeded,
        },
    },
    bevy::{
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            system::Local,
        },
        time::{Time, Timer, TimerMode},
    },
    bevy_ratatui::RatatuiContext,
    ratatui::{
        Frame,
        layout::{Constraint, Layout},
        style::{Color, Style},
        text::{Line, Span},
        widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation},
    },
    std::time::Duration,
    unicode_width::{UnicodeWidthChar, UnicodeWidthStr},
};

/// Bevy system: decides whether to render a frame and drives periodic timers.
///
/// # Timers
///
/// | Timer | Period | Effect |
/// |-------|--------|--------|
/// | `cursor_timer` | [`CURSOR_BLINK_INTERVAL_MS`] ms | Toggles `TuiMain::show_cursor` |
/// | `spinner_timer` | 100 ms | Forces a redraw while any block has a running animation |
///
/// Both timers are `Local<Option<Timer>>` so they survive across frames without
/// needing a Bevy resource.  The `Option` lets us lazily initialize them on the
/// first call.
///
/// [`CURSOR_BLINK_INTERVAL_MS`]: crate::tui::core::CURSOR_BLINK_INTERVAL_MS
pub fn draw_scene_system(
    mut context: ResMut<RatatuiContext>,
    mut tui: NonSendMut<TuiMain>,
    time: Res<Time<()>>,
    mut cursor_timer: Local<Option<Timer>>,
    mut spinner_timer: Local<Option<Timer>>,
    mut dirty: ResMut<RenderNeeded>,
    token_usage: Option<Res<CodingAgentTotalTokenUsage>>,
) -> bevy::ecs::error::Result {
    // ── Cursor animation ──────────────────────────────────────────────────────
    match tui.cursor_style {
        CursorStyle::Blink => {
            let cursor_timer = cursor_timer.get_or_insert(Timer::new(
                Duration::from_millis(CURSOR_BLINK_INTERVAL_MS),
                TimerMode::Repeating,
            ));
            _ = cursor_timer.tick(time.delta());
            if cursor_timer.just_finished() {
                tui.show_cursor ^= true;
                **dirty = true;
            }
        }
        CursorStyle::Breathing => {
            // Advance phase proportional to elapsed time. Full cycle = 5 seconds.
            tui.cursor_phase = (tui.cursor_phase + time.delta().as_secs_f32() / 5.0) % 1.0;
            **dirty = true;
        }
    }

    // ── Spinner refresh ───────────────────────────────────────────────────────
    // Throttle redraws to 10 fps while animations are running.
    let spinner_timer =
        spinner_timer.get_or_insert(Timer::new(Duration::from_millis(100), TimerMode::Repeating));
    _ = spinner_timer.tick(time.delta());
    // Only force redraws when at least one block is actively animating.
    let has_spinner = tui.blocks.iter().any(|b| match b {
        crate::tui::core::OutputBlock::Response(resp) => resp.has_spinner(),
        _ => false,
    });
    if has_spinner && spinner_timer.just_finished() {
        **dirty = true;
    }

    // ── Render ────────────────────────────────────────────────────────────────
    if **dirty {
        _ = context.draw(|frame| {
            () = render_tui(frame, &mut tui, token_usage.as_deref());
        })?;
    }
    **dirty = false;
    Ok(())
}

/// Lays out and paints the three-panel UI (Output / Status / Input).
///
/// Layout (top → bottom):
/// * **Output** — takes all remaining vertical space (`Min(3)`)
/// * **Status** — fixed 3-row token-usage bar
/// * **Input** — grows dynamically with the number of wrapped input lines
///
/// The layout is recomputed from scratch every frame so terminal resizes and
/// multi-line input are handled automatically.
fn render_tui(
    frame: &mut Frame,
    tui: &mut TuiMain,
    token_usage: Option<&CodingAgentTotalTokenUsage>,
) {
    let area = frame.area();

    // ── Layout ────────────────────────────────────────────────────────────────
    // Pre-compute input line count so the input panel height is exact.
    let inner_width = area.width.saturating_sub(2) as usize;
    let input_lines = display_utils::input_display_lines(tui, inner_width, PROMPT_PREFIX);
    // Minimum 3 rows = 1 content row + 2 border rows.
    let input_height = (input_lines.len() as u16 + 2).max(3);
    let vertical = Layout::vertical([
        Constraint::Min(3),               // output — fills remaining space
        Constraint::Length(3),            // status bar — always 3 rows
        Constraint::Length(input_height), // input — dynamic
    ]);
    let [output_area, status_area, input_area] = vertical.areas(area);

    // Store output_area so scroll math and mouse hit-testing stay in sync.
    tui.output_area = output_area;

    // ── Output rendering ──────────────────────────────────────────────────────
    let output_inner_width = output_area.width.saturating_sub(2) as usize;
    // Flatten all OutputBlocks → (Line, ClickAction) pairs.
    let (flat, flat_map) = display_utils::rendered_flat_lines(tui, &SPINNER);
    // Word-wrap both lists in lockstep to preserve the line→block mapping.
    let (wrapped, wrapped_map) =
        display_utils::hard_wrap_output_lines_with_map(&flat, &flat_map, output_inner_width);
    // Persist the wrapped map so handle_mouse_input can look up click actions.
    tui.line_map = wrapped_map;

    let total_rows = wrapped.len();
    let output_height = output_area.height.saturating_sub(2) as usize;
    // Clamp scroll in case content was removed (e.g. /clear).
    let new_max = total_rows.saturating_sub(output_height);
    tui.vertical_scroll = tui.vertical_scroll.min(new_max);

    // Focused panel → white border; unfocused → dark gray.
    let output_border_color = if tui.focused == TuiMainFocus::OutputArea {
        COLOR_BORDER_FOCUSED
    } else {
        COLOR_BORDER_UNFOCUSED
    };
    let output = Paragraph::new(wrapped)
        .style(Style::default())
        .block(
            Block::bordered()
                .title("Output")
                .border_style(Style::default().fg(output_border_color)),
        )
        .scroll((tui.vertical_scroll as u16, 0));

    // Update scrollbar state so the thumb position tracks the scroll offset.
    let scroll_positions = total_rows.saturating_sub(output_height) + 1;
    tui.vertical_scroll_state = tui
        .vertical_scroll_state
        .content_length(scroll_positions)
        .viewport_content_length(output_height)
        .position(tui.vertical_scroll);
    () = frame.render_widget(output, output_area);
    () = frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        output_area,
        &mut tui.vertical_scroll_state,
    );

    // ── Status bar ────────────────────────────────────────────────────────────
    let status_text = if let Some(usage) = token_usage {
        let CodingAgentTotalTokenUsage(usage) = usage;
        format!(
            " 🎯 Input: {} | Output: {} | Cache Read: {} | Cache Write: {}",
            usage.input, usage.output, usage.cache_read, usage.cache_write
        )
    } else {
        " 🎯 Token usage: Waiting for first response...".to_string()
    };
    let status = Paragraph::new(status_text)
        .style(Style::default().fg(ratatui::style::Color::Rgb(100, 150, 200)))
        .block(Block::bordered().title("Token Usage"));
    () = frame.render_widget(status, status_area);

    // ── Input panel ───────────────────────────────────────────────────────────
    let is_input_focused = tui.focused == TuiMainFocus::InputArea;
    let cursor_total = PROMPT_PREFIX.width() + display_utils::display_index(tui);
    let input_border_color = if is_input_focused {
        COLOR_BORDER_FOCUSED
    } else {
        COLOR_BORDER_UNFOCUSED
    };

    let input_text: Vec<Line<'_>> = match tui.cursor_style {
        CursorStyle::Breathing if is_input_focused => {
            breathing_input_lines(&input_lines, cursor_total, tui.cursor_phase)
        }
        _ => input_lines.iter().map(|l| Line::from(l.as_str())).collect(),
    };

    let input = Paragraph::new(input_text).style(Style::default()).block(
        Block::bordered()
            .title("Input")
            .border_style(Style::default().fg(input_border_color)),
    );
    () = frame.render_widget(input, input_area);

    // ── Software cursor ───────────────────────────────────────────────────────
    // Blink mode: use the terminal's native cursor (set_cursor_position).
    // Breathing mode: cursor is drawn as a styled span inside the paragraph above.
    if tui.cursor_style == CursorStyle::Blink && tui.show_cursor && is_input_focused {
        let (cursor_row, cursor_col) = {
            let mut accumulated = 0usize;
            let mut result = (0usize, cursor_total);
            for (row, line) in input_lines.iter().enumerate() {
                let line_w = line.width();
                if cursor_total <= accumulated + line_w {
                    result = (row, cursor_total - accumulated);
                    break;
                }
                if row + 1 < input_lines.len() {
                    accumulated += line_w;
                } else {
                    result = (row, cursor_total - accumulated);
                }
            }
            result
        };
        () = frame.set_cursor_position((
            input_area.left() + cursor_col as u16 + 1,
            input_area.top() + cursor_row as u16 + 1,
        ));
    }
}

/// Computes a normalised brightness value in `[0.0, 1.0]` for the current breathing `phase`.
///
/// # Background — Apple Breathing LED
///
/// Introduced with the PowerBook G4 (2001), the Sleep Indicator LED on Apple portables
/// pulses with a distinctive asymmetric rhythm that Apple engineers deliberately tuned
/// to mimic the average human resting breathing rate (~12 breaths per minute, i.e. a
/// ~5-second cycle).
///
/// The waveform is **not** a symmetric sine wave.  It uses two Gaussian half-bells
/// joined at the peak — one narrow (inhale) and one wide (exhale):
///
/// ```text
///         1.0 ┤        ╭╮
///             │       ╭  ╮
///             │      ╭    ╮
///             │     ╭      ╮
///             │    ╭        ╮
///    0.05 ┤───╯               ╰──────────────╮  (5 % floor)
///             0   0.8 s      ←── 5 s ──→
///                peak     exhale tail + pause
/// ```
///
/// - **Inhale (rise)**: narrow Gaussian (small σ) — LED brightens quickly.
/// - **Exhale (fall)**: wide Gaussian (large σ) — LED dims slowly.
/// - **Pause**: the long tail of the exhale Gaussian naturally creates a dark
///   "rest" interval before the next inhale; no explicit pause constant is needed.
///
/// # Mathematical form
///
/// Asymmetric (split-normal) Gaussian, normalised so `f(μ) = 1.0`:
///
/// ```text
/// f(t) = exp( −(t − μ)² / (2σ²) )
/// ```
///
/// σ switches at `t = μ`:
///
/// | Parameter | Value  | Meaning                              |
/// |-----------|--------|--------------------------------------|
/// | period    | 5.0 s  | full inhale → exhale → pause cycle   |
/// | μ         | 0.8 s  | peak centre (LED at maximum)         |
/// | σ\_rise   | 0.25 s | narrow → fast inhale                 |
/// | σ\_fall   | 1.5 s  | wide  → slow exhale + natural pause  |
fn breathing_brightness(phase: f32) -> f32 {
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
/// `fg(Color::Black)` was previously used unconditionally.  On a dark terminal this
/// makes the cursor character **invisible** during the dark phase (black text on a
/// near-black background).  The fix uses the brightness threshold to pick a
/// contrasting foreground:
///
/// - brightness > 50 % → `fg(Color::Black)` — dark text on bright background.
/// - brightness ≤ 50 % → `fg(Color::White)` — bright text on dark background.
fn breathing_cursor_style(phase: f32) -> Style {
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

/// Builds styled [`Line`]s for the input box in [`CursorStyle::Breathing`] mode.
///
/// Only the single character cell under the cursor receives a style; every other
/// character is left as a plain [`Span::raw`] so the breathing effect is strictly
/// isolated to the cursor position.
fn breathing_input_lines(
    lines: &[String],
    cursor_col_total: usize,
    phase: f32,
) -> Vec<Line<'static>> {
    let cursor_style = breathing_cursor_style(phase);
    let mut accumulated = 0usize;
    let mut cursor_placed = false;
    let mut result: Vec<Line<'static>> = Vec::with_capacity(lines.len());

    #[allow(clippy::collapsible_if)]
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

    // Cursor past all lines (empty input or cursor after every character).
    #[allow(clippy::collapsible_if)]
    if !cursor_placed {
        if let Some(last) = result.last_mut() {
            last.spans.push(Span::styled(" ", cursor_style));
        }
    }

    result
}
