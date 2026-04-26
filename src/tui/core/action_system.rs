//! Action dispatch system — translates [`TuiAction`] messages into mutations
//! on [`TuiMain`].
//!
//! This is the single place where business logic lives.  Input handlers are
//! kept thin (event → message); this system owns the "what does it mean" layer.
//!
//! [`TuiAction`]: crate::tui::events::TuiAction

use {
    crate::{
        agents::CodingAgentPromptChannel,
        tui::{
            commands,
            core::{CursorState, OutputBlock, TuiMain, TuiMainFocus},
            events::{RenderNeeded, TuiAction},
        },
    },
    bevy::app::AppExit,
    bevy::ecs::{
        change_detection::{NonSendMut, Res, ResMut},
        message::{MessageReader, MessageWriter},
    },
    bevy::state::state::NextState,
    ratatui::{style::Stylize, text::Line},
};

/// Reads all pending [`TuiAction`] messages and applies each one to [`TuiMain`].
///
/// Every branch sets `**dirty = true` to schedule a redraw; the renderer skips
/// a frame if `dirty` is still `false` from the previous tick.
pub fn tui_action_system(
    mut actions: MessageReader<TuiAction>,
    mut tui: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut exit: MessageWriter<AppExit>,
    mut next_cursor_state: ResMut<NextState<CursorState>>,
    channel: Res<CodingAgentPromptChannel>,
) {
    for action_msg in actions.read() {
        // Any user interaction resets the idle timer and switches back to Blink mode.
        tui.reset_activity();
        next_cursor_state.set(CursorState::Blink);

        let action = action_msg;
        match action {
            TuiAction::InsertChar(c) => {
                () = tui.insert_char(*c);
                **dirty = true;
            }
            TuiAction::Backspace => {
                () = tui.delete_before();
                **dirty = true;
            }
            TuiAction::Delete => {
                () = tui.delete_after();
                **dirty = true;
            }
            TuiAction::CursorLeft => {
                () = tui.cursor_left();
                **dirty = true;
            }
            TuiAction::CursorRight => {
                () = tui.cursor_right();
                **dirty = true;
            }
            TuiAction::CursorToStart => {
                if tui.focused == TuiMainFocus::InputArea {
                    () = tui.cursor_to_start();
                } else {
                    () = tui.scroll_to_top();
                }
                **dirty = true;
            }
            TuiAction::CursorToEnd => {
                if tui.focused == TuiMainFocus::InputArea {
                    () = tui.cursor_to_end();
                } else {
                    () = tui.scroll_to_bottom();
                }
                **dirty = true;
            }
            TuiAction::HistoryPrev => {
                () = tui.history_prev();
                **dirty = true;
            }
            TuiAction::HistoryNext => {
                () = tui.history_next();
                **dirty = true;
            }
            TuiAction::ScrollUp => {
                () = tui.scroll_up();
                **dirty = true;
            }
            TuiAction::ScrollDown => {
                () = tui.scroll_down();
                **dirty = true;
            }
            TuiAction::ToggleLastThinking => {
                () = tui.toggle_last_thinking();
                **dirty = true;
            }
            TuiAction::ExpandAllThinking => {
                () = tui.expand_all_thinking();
                **dirty = true;
            }
            TuiAction::CollapseAllThinking => {
                () = tui.collapse_all_thinking();
                **dirty = true;
            }
            TuiAction::Submit => {
                handle_submit(&mut tui, &mut exit, &channel);
                // Scroll to bottom so the user always sees their newly submitted prompt.
                () = tui.scroll_to_bottom();
                **dirty = true;
            }
            TuiAction::ToggleThinking {
                block_index,
                thinking_index,
            } => {
                let bi = *block_index;
                let ti = *thinking_index;
                // Only toggle if the block is fully received (not still streaming).
                if let Some(tb) = tui
                    .blocks
                    .get_mut(bi)
                    .and_then(|b| {
                        if let OutputBlock::Response(resp) = b {
                            resp.thinkings.get_mut(ti)
                        } else {
                            None
                        }
                    })
                    .filter(|tb| !tb.streaming)
                {
                    tb.expanded = !tb.expanded;
                }
                // Clicking a thinking header also selects its parent block.
                tui.selected_block = Some(bi);
                **dirty = true;
            }
            TuiAction::SelectBlock(idx) => {
                let i = *idx;
                // Clicking the same block again deselects it (toggle).
                tui.selected_block = if tui.selected_block == Some(i) {
                    None
                } else {
                    Some(i)
                };
                **dirty = true;
            }
            TuiAction::DeselectBlock => {
                tui.selected_block = None;
                **dirty = true;
            }
        }
    }
}

/// Processes a submitted input line.
///
/// Dispatch order:
/// 1. `/exit` / `/quit` → write [`AppExit`] message and clear input.
/// 2. `/clear` → clear all output and input.
/// 3. Any other `/…` command → route through [`commands::handle_slash_command`].
/// 4. Plain text → push a dim echo line, clear input, and send to the agent
///    via [`CodingAgentPromptChannel`].
fn handle_submit(
    tui: &mut TuiMain,
    exit: &mut MessageWriter<AppExit>,
    channel: &CodingAgentPromptChannel,
) {
    if tui.input.is_empty() {
        return;
    }
    let input = tui.input.clone();
    let trimmed = input.trim();
    match trimmed {
        "/exit" | "/quit" => {
            () = tui.clear_input();
            _ = exit.write_default();
        }
        "/clear" => {
            () = tui.clear_output();
            () = tui.clear_input();
        }
        cmd if cmd.starts_with('/') => {
            let lines = commands::handle_slash_command(cmd);
            () = tui.push_history(&input);
            () = tui.push_lines(lines);
            () = tui.clear_input();
        }
        _ => {
            // Echo the prompt in dim gray so the user can see what they sent.
            () = tui.push_history(&input);
            () = tui.push_line(Line::from(format!("> {input}")).dark_gray());
            () = tui.clear_input();
            // Forward to the agent over the async channel.
            match channel.sender.send(input) {
                Ok(_) => {}
                Err(e) => {
                    let err_line = Line::from(format!("❌ Failed to send input: {e}")).red();
                    () = tui.push_line(err_line);
                }
            }
        }
    }
}
