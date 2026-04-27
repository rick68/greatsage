//! Keyboard and mouse input handlers.
//!
//! Each handler translates raw crossterm events into [`TuiAction`] messages.
//! The messages are consumed by [`crate::tui::core::action_system`] the same
//! Bevy tick.
//!
//! # Handler responsibilities
//!
//! | Function | Runs when | Covers |
//! |----------|-----------|--------|
//! | [`handle_global_input`] | Always (PreUpdate) | Tab, Ctrl-C, scroll, `t`/`a`/`A` |
//! | [`handle_input_area_input`] | Only in `InputArea` state | Text editing, history, Enter |
//! | [`handle_mouse_input`] | Always (PreUpdate) | Scroll wheel, left click |
//!
//! [`TuiAction`]: crate::tui::events::TuiAction

use {
    crate::tui::{
        core::{TuiMain, TuiMainFocus},
        events::{RenderNeeded, TuiAction},
    },
    bevy::{
        app::AppExit,
        ecs::{
            change_detection::{DetectChangesMut, NonSendMut, Res, ResMut},
            message::MessageWriter,
        },
        state::state::{NextState, State},
    },
    bevy_ratatui::{
        crossterm,
        event::{KeyMessage, MouseMessage},
    },
};

/// Handles keyboard events that are active regardless of which panel is focused.
///
/// # Key bindings
///
/// | Key | Condition | Action |
/// |-----|-----------|--------|
/// | Tab | always | Cycle focus (InputArea ↔ OutputArea) |
/// | F10 | always | Exit the application |
/// | Double-Esc | always | Exit the application |
/// | Ctrl-C | selection active | Copy selection |
/// | ↑ / ↓ | OutputArea only | Scroll output one line |
/// | PgUp / PgDn | always | Scroll output one line |
/// | Home | OutputArea only | Scroll to top |
/// | End | OutputArea only | Scroll to bottom |
/// | `t` | OutputArea only | Toggle last (or selected) thinking block |
/// | `A` | OutputArea only | Expand **all** thinking blocks |
/// | `a` | OutputArea only | Collapse **all** thinking blocks |
///
/// The `t`/`a`/`A` keys require OutputArea focus so they don't interfere with
/// typing those characters in the input box.
pub fn handle_global_input(
    mut messages: bevy::ecs::message::MessageReader<KeyMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut next_tui_main_focus: ResMut<NextState<TuiMainFocus>>,
    _exit: MessageWriter<AppExit>,
    focus: Res<State<TuiMainFocus>>,
    mut actions: MessageWriter<TuiAction>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    for message in messages.read() {
        let KeyEvent {
            code,
            kind,
            modifiers,
            ..
        } = &**message;
        // True when the text-entry box owns the keyboard.
        let in_input = focus.get() == &TuiMainFocus::InputArea;
        match code {
            KeyCode::F(10) => {
                _ = actions.write(TuiAction::Quit);
            }
            KeyCode::Esc => {
                let now = std::time::Instant::now();
                let last_esc = tui_main.last_click;
                if let Some((last_time, (999, 999), 0)) = last_esc
                    && now.duration_since(last_time).as_millis() < 500
                {
                    _ = actions.write(TuiAction::Quit);
                }
                tui_main.bypass_change_detection().last_click = Some((now, (999, 999), 0));
                **dirty = true;
            }
            // Tab cycles focus; updates the Bevy state so run-conditions apply.
            KeyCode::Tab => {
                let mut current = *focus.get();
                let next = current.next().unwrap();
                next_tui_main_focus.set(next);
                tui_main.focused = next;
                **dirty = true;
            }
            // Ctrl-C (all platforms) or Cmd-C (macOS) copies selection if active.
            KeyCode::Char('c')
                if matches!(kind, KeyEventKind::Press)
                    && (modifiers.contains(KeyModifiers::CONTROL)
                        || modifiers.contains(KeyModifiers::SUPER)) =>
            {
                if tui_main.selection.is_some() {
                    _ = actions.write(TuiAction::CopySelection);
                } else {
                    **dirty = true;
                }
            }
            // Arrow keys scroll only when the output panel is focused; in the
            // input panel they navigate command history (handled separately).
            KeyCode::Up
                if !in_input && (kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat) =>
            {
                _ = actions.write(TuiAction::ScrollUp);
            }
            KeyCode::Down
                if !in_input && (kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat) =>
            {
                _ = actions.write(TuiAction::ScrollDown);
            }
            // Page keys scroll in both panels.
            KeyCode::PageUp => {
                _ = actions.write(TuiAction::ScrollUp);
            }
            KeyCode::PageDown => {
                _ = actions.write(TuiAction::ScrollDown);
            }
            // Home/End jump to extremes of the output; in the input they move
            // the cursor (handled by handle_input_area_input).
            KeyCode::Home if !in_input => {
                _ = actions.write(TuiAction::CursorToStart);
            }
            KeyCode::End if !in_input => {
                _ = actions.write(TuiAction::CursorToEnd);
            }
            // Thinking-block controls — only when not editing text.
            // `t`: toggle the selected (or last) thinking block.
            KeyCode::Char('t') if !in_input && matches!(kind, KeyEventKind::Press) => {
                _ = actions.write(TuiAction::ToggleLastThinking);
            }
            // `A` (capital): expand every thinking block.
            KeyCode::Char('A') if !in_input && matches!(kind, KeyEventKind::Press) => {
                _ = actions.write(TuiAction::ExpandAllThinking);
            }
            // `a` (lowercase): collapse every thinking block.
            KeyCode::Char('a') if !in_input && matches!(kind, KeyEventKind::Press) => {
                _ = actions.write(TuiAction::CollapseAllThinking);
            }
            _ => (),
        }
    }
}

/// Handles keyboard events specific to the input text box.
///
/// Only active when [`TuiMainFocus::InputArea`] is the current Bevy state.
///
/// # Key bindings
///
/// | Key | Action |
/// |-----|--------|
/// | printable char (no Ctrl) | Insert character |
/// | Ctrl-A | Cursor to start (Emacs-style) |
/// | Ctrl-E | Cursor to end (Emacs-style) |
/// | Backspace | Delete before cursor |
/// | Delete | Delete after cursor |
/// | ← / → | Move cursor one scalar left/right |
/// | Home / End | Cursor to start/end |
/// | ↑ / ↓ | Navigate command history |
/// | Enter | Submit input |
pub fn handle_input_area_input(
    mut messages: bevy::ecs::message::MessageReader<KeyMessage>,
    mut actions: MessageWriter<TuiAction>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    for message in messages.read() {
        let KeyEvent {
            code,
            kind,
            modifiers,
            ..
        } = &**message;
        // Accept both Press and Repeat for keys that should auto-repeat when held.
        let is_active = kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat;
        match code {
            // Regular character input; skip Ctrl-modified chars (they have dedicated bindings).
            KeyCode::Char(c) if is_active && !modifiers.contains(KeyModifiers::CONTROL) => {
                _ = actions.write(TuiAction::InsertChar(*c));
            }
            // Emacs-style line-start shortcut (Ctrl-A).
            KeyCode::Char('a')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                _ = actions.write(TuiAction::CursorToStart);
            }
            // Emacs-style line-end shortcut (Ctrl-E).
            KeyCode::Char('e')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                _ = actions.write(TuiAction::CursorToEnd);
            }
            KeyCode::Backspace if is_active => {
                _ = actions.write(TuiAction::Backspace);
            }
            KeyCode::Delete if is_active => {
                _ = actions.write(TuiAction::Delete);
            }
            KeyCode::Left if is_active => {
                _ = actions.write(TuiAction::CursorLeft);
            }
            KeyCode::Right if is_active => {
                _ = actions.write(TuiAction::CursorRight);
            }
            KeyCode::Home if matches!(kind, KeyEventKind::Press) => {
                _ = actions.write(TuiAction::CursorToStart);
            }
            KeyCode::End if matches!(kind, KeyEventKind::Press) => {
                _ = actions.write(TuiAction::CursorToEnd);
            }
            // History navigation (no auto-repeat to avoid runaway history traversal).
            KeyCode::Up if matches!(kind, KeyEventKind::Press) => {
                _ = actions.write(TuiAction::HistoryPrev);
            }
            KeyCode::Down if matches!(kind, KeyEventKind::Press) => {
                _ = actions.write(TuiAction::HistoryNext);
            }
            KeyCode::Enter if matches!(kind, KeyEventKind::Press) => {
                _ = actions.write(TuiAction::Submit);
            }
            _ => (),
        }
    }
}

/// Handles mouse events from the terminal.
///
/// # Behaviour
///
/// * **Scroll wheel** — emits [`TuiAction::ScrollUp`] / [`TuiAction::ScrollDown`]
///   regardless of where the cursor is on screen.
/// * **Left click inside the output area** — looks up the clicked visual row in
///   [`TuiMain::line_map`] and emits either:
///   * [`TuiAction::ToggleThinking`] if the row is a thinking-block header (▶/▼), or
///   * [`TuiAction::SelectBlock`] if the row belongs to a response block body.
///
/// Clicks outside the output area are silently ignored.
pub fn handle_mouse_input(
    mut messages: bevy::ecs::message::MessageReader<MouseMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut actions: MessageWriter<TuiAction>,
) {
    use bevy_ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Position;
    for message in messages.read() {
        let mouse_event: &MouseEvent = message;
        let row = mouse_event.row;
        let column = mouse_event.column;
        let kind = mouse_event.kind;
        match kind {
            MouseEventKind::ScrollUp => {
                _ = actions.write(TuiAction::ScrollUp);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let output_area = tui_main.output_area;
                if output_area.contains(ratatui::layout::Position { x: column, y: row }) {
                    let inner_row = (row).saturating_sub(output_area.top() + 1) as usize;
                    let map_row = inner_row + tui_main.vertical_scroll;
                    let inner_width = output_area.width.saturating_sub(2) as usize;
                    let rel_col = (column).saturating_sub(output_area.left() + 1) as usize;
                    let logical_col = rel_col.min(inner_width);
                    if let Some(selection) = tui_main.selection {
                        _ = actions.write(TuiAction::SetSelection {
                            start: selection.start,
                            end: (map_row, logical_col),
                            click_count: 1,
                        });
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                _ = actions.write(TuiAction::ScrollDown);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let now = std::time::Instant::now();
                let mut click_count = 1;
                if let Some((last_time, last_pos, last_count)) = tui_main.last_click
                    && now.duration_since(last_time).as_millis() < 500
                    && last_pos == (row as usize, column as usize)
                {
                    click_count = (last_count % 3) + 1;
                }
                tui_main.bypass_change_detection().last_click =
                    Some((now, (row as usize, column as usize), click_count));
                let output_area = tui_main.output_area;
                // Only act on clicks that land inside the output panel.
                if !output_area.contains(Position { x: column, y: row }) {
                    if tui_main.selection.is_some() {
                        _ = actions.write(TuiAction::ClearSelection);
                    }
                } else {
                    // Convert terminal row to an inner row (subtract border + top offset).
                    let inner_row = (row).saturating_sub(output_area.top() + 1) as usize;
                    // Add current scroll offset to get the absolute logical row index.
                    let map_row = inner_row + tui_main.vertical_scroll;
                    // Look up the action associated with this row in the line map.
                    let inner_width = output_area.width.saturating_sub(2) as usize;
                    let rel_col = (column).saturating_sub(output_area.left() + 1) as usize;
                    let logical_col = rel_col.min(inner_width);
                    _ = actions.write(TuiAction::SetSelection {
                        start: (map_row, logical_col),
                        end: (map_row, logical_col),
                        click_count,
                    });
                    if let Some(Some((block_idx, click_action))) =
                        tui_main.line_map.get(map_row).copied()
                    {
                        match click_action {
                            // Clicking a thinking header (▶/▼) toggles that specific block.
                            crate::tui::core::ClickAction::ToggleThinking(ti) => {
                                actions.write(TuiAction::ToggleThinking {
                                    block_index: block_idx,
                                    thinking_index: ti,
                                });
                            }
                            // Clicking anywhere else in a response block selects/deselects it.
                            crate::tui::core::ClickAction::Select => {
                                _ = actions.write(TuiAction::SelectBlock(block_idx));
                            }
                        }
                    }
                }
            }
            _ => (),
        }
    }
}
