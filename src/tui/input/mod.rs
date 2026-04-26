use bevy_ratatui::crossterm;
use {
    crate::tui::{
        core::{TuiMain, TuiMainFocus},
        events::{RenderNeeded, TuiAction},
    },
    bevy::app::AppExit,
    bevy::ecs::{
        change_detection::{NonSendMut, Res, ResMut},
        message::MessageWriter,
    },
    bevy::state::state::{NextState, State},
    bevy_ratatui::event::{KeyMessage, MouseMessage},
};

pub fn handle_global_input(
    mut messages: bevy::ecs::message::MessageReader<KeyMessage>,
    tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut next_tui_main_focus: ResMut<NextState<TuiMainFocus>>,
    mut exit: MessageWriter<AppExit>,
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
        let in_input = focus.get() == &TuiMainFocus::InputArea;
        match code {
            KeyCode::Tab => {
                let mut current = tui_main.focused;
                let next = current.next().unwrap();
                next_tui_main_focus.set(next);
                **dirty = true;
            }
            KeyCode::Char('c')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                exit.write_default();
            }
            KeyCode::Up
                if !in_input && (kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat) =>
            {
                actions.write(TuiAction::ScrollUp);
            }
            KeyCode::Down
                if !in_input && (kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat) =>
            {
                actions.write(TuiAction::ScrollDown);
            }
            KeyCode::PageUp => {
                actions.write(TuiAction::ScrollUp);
            }
            KeyCode::PageDown => {
                actions.write(TuiAction::ScrollDown);
            }
            KeyCode::Home if !in_input => {
                actions.write(TuiAction::CursorToStart);
            }
            KeyCode::End if !in_input => {
                actions.write(TuiAction::CursorToEnd);
            }
            KeyCode::Char('t') if !in_input && matches!(kind, KeyEventKind::Press) => {
                actions.write(TuiAction::ToggleLastThinking);
            }
            KeyCode::Char('A') if !in_input && matches!(kind, KeyEventKind::Press) => {
                actions.write(TuiAction::ExpandAllThinking);
            }
            KeyCode::Char('a') if !in_input && matches!(kind, KeyEventKind::Press) => {
                actions.write(TuiAction::CollapseAllThinking);
            }
            _ => (),
        }
    }
}

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
        let is_active = kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat;
        match code {
            KeyCode::Char(c) if is_active && !modifiers.contains(KeyModifiers::CONTROL) => {
                actions.write(TuiAction::InsertChar(*c));
            }
            KeyCode::Char('a')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                actions.write(TuiAction::CursorToStart);
            }
            KeyCode::Char('e')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                actions.write(TuiAction::CursorToEnd);
            }
            KeyCode::Backspace if is_active => {
                actions.write(TuiAction::Backspace);
            }
            KeyCode::Delete if is_active => {
                actions.write(TuiAction::Delete);
            }
            KeyCode::Left if is_active => {
                actions.write(TuiAction::CursorLeft);
            }
            KeyCode::Right if is_active => {
                actions.write(TuiAction::CursorRight);
            }
            KeyCode::Home if matches!(kind, KeyEventKind::Press) => {
                actions.write(TuiAction::CursorToStart);
            }
            KeyCode::End if matches!(kind, KeyEventKind::Press) => {
                actions.write(TuiAction::CursorToEnd);
            }
            KeyCode::Up if matches!(kind, KeyEventKind::Press) => {
                actions.write(TuiAction::HistoryPrev);
            }
            KeyCode::Down if matches!(kind, KeyEventKind::Press) => {
                actions.write(TuiAction::HistoryNext);
            }
            KeyCode::Enter if matches!(kind, KeyEventKind::Press) => {
                actions.write(TuiAction::Submit);
            }
            _ => (),
        }
    }
}

pub fn handle_mouse_input(
    mut messages: bevy::ecs::message::MessageReader<MouseMessage>,
    tui_main: NonSendMut<TuiMain>,
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
                actions.write(TuiAction::ScrollUp);
            }
            MouseEventKind::ScrollDown => {
                actions.write(TuiAction::ScrollDown);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let output_area = tui_main.output_area;
                if output_area.contains(Position { x: column, y: row }) {
                    let inner_row = (row).saturating_sub(output_area.top() + 1) as usize;
                    let map_row = inner_row + tui_main.vertical_scroll;
                    if let Some(Some((block_idx, click_action))) =
                        tui_main.line_map.get(map_row).copied()
                    {
                        match click_action {
                            crate::tui::core::ClickAction::ToggleThinking(ti) => {
                                actions.write(TuiAction::ToggleThinking {
                                    block_index: block_idx,
                                    thinking_index: ti,
                                });
                            }
                            crate::tui::core::ClickAction::Select => {
                                actions.write(TuiAction::SelectBlock(block_idx));
                            }
                        }
                    }
                }
            }
            _ => (),
        }
    }
}
