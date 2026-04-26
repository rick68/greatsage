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

pub mod cursor;
pub mod display_utils;
pub mod widgets;

use {
    crate::{
        agents::CodingAgentTotalTokenUsage,
        tui::{
            core::{
                COLOR_BORDER_FOCUSED, COLOR_BORDER_UNFOCUSED, CURSOR_BLINK_INTERVAL_MS,
                CursorState, PROMPT_PREFIX, SPINNER, TuiMain, TuiMainFocus,
            },
            events::RenderNeeded,
            renderer::cursor::breathing_input_lines,
        },
    },
    bevy::{
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            system::Local,
        },
        state::state::{NextState, State},
        time::{Time, Timer, TimerMode},
    },
    bevy_ratatui::RatatuiContext,
    ratatui::{
        Frame,
        layout::{Constraint, Layout},
        style::Style,
        text::Line,
        widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation},
    },
    std::time::Duration,
    unicode_width::UnicodeWidthStr,
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
/// # Idle timeout
///
/// If no user activity is detected for `tui.cursor_idle_to_breathing_ms`, the
/// system transitions `CursorState` to `Breathing`.
///
/// [`CURSOR_BLINK_INTERVAL_MS`]: crate::tui::core::CURSOR_BLINK_INTERVAL_MS
pub fn draw_scene_system(
    mut context: ResMut<RatatuiContext>,
    mut tui: NonSendMut<TuiMain>,
    time: Res<Time<()>>,
    cursor_state: Res<State<CursorState>>,
    mut next_cursor_state: ResMut<NextState<CursorState>>,
    mut cursor_timer: Local<Option<Timer>>,
    mut spinner_timer: Local<Option<Timer>>,
    mut dirty: ResMut<RenderNeeded>,
    token_usage: Option<Res<CodingAgentTotalTokenUsage>>,
    config: Res<crate::config::AppConfig>,
) -> bevy::ecs::error::Result {
    // ── Cursor animation & Idle timeout ───────────────────────────────────────
    match cursor_state.get() {
        CursorState::Blink => {
            let cursor_timer = cursor_timer.get_or_insert(Timer::new(
                Duration::from_millis(CURSOR_BLINK_INTERVAL_MS),
                TimerMode::Repeating,
            ));
            _ = cursor_timer.tick(time.delta());
            if cursor_timer.just_finished() {
                tui.show_cursor ^= true;
                **dirty = true;
            }

            // Transition to Breathing after idle timeout.
            let idle_time = tui.last_activity.elapsed();
            if idle_time.as_millis() >= config.tui.cursor_idle_to_breathing_ms as u128 {
                next_cursor_state.set(CursorState::Breathing);
            }
        }
        CursorState::Breathing => {
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
            () = render_tui(frame, &mut tui, &cursor_state, token_usage.as_deref());
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
    cursor_state: &State<CursorState>,
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

    let input_text: Vec<Line<'_>> = match cursor_state.get() {
        CursorState::Breathing if is_input_focused => {
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
    if cursor_state.get() == &CursorState::Blink && tui.show_cursor && is_input_focused {
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
