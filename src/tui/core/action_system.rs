use {
    crate::{
        agents::CodingAgentPromptChannel,
        tui::{
            commands,
            core::{OutputBlock, TuiMain, TuiMainFocus},
            events::{RenderNeeded, TuiAction},
        },
    },
    bevy::ecs::{
        change_detection::{NonSendMut, Res, ResMut},
        message::{MessageReader, MessageWriter},
    },
    bevy::app::AppExit,
    ratatui::{
        prelude::Stylize,
        text::Line,
    },
};

/// Listens for TuiAction events and performs corresponding data operations on TuiMain.
pub fn tui_action_system(
    mut actions: MessageReader<TuiAction>,
    mut tui: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut exit: MessageWriter<AppExit>,
    channel: Res<CodingAgentPromptChannel>,
) {
    for action_msg in actions.read() {
        let action = action_msg;
        match action {
            TuiAction::InsertChar(c) => {
                tui.insert_char(*c);
                **dirty = true;
            }
            TuiAction::Backspace => {
                tui.delete_before();
                **dirty = true;
            }
            TuiAction::Delete => {
                tui.delete_after();
                **dirty = true;
            }
            TuiAction::CursorLeft => {
                tui.cursor_left();
                **dirty = true;
            }
            TuiAction::CursorRight => {
                tui.cursor_right();
                **dirty = true;
            }
            TuiAction::CursorToStart => {
                if tui.focused == TuiMainFocus::InputArea {
                    tui.cursor_to_start();
                } else {
                    tui.scroll_to_top();
                }
                **dirty = true;
            }
            TuiAction::CursorToEnd => {
                if tui.focused == TuiMainFocus::InputArea {
                    tui.cursor_to_end();
                } else {
                    tui.scroll_to_bottom();
                }
                **dirty = true;
            }
            TuiAction::HistoryPrev => {
                tui.history_prev();
                **dirty = true;
            }
            TuiAction::HistoryNext => {
                tui.history_next();
                **dirty = true;
            }
            TuiAction::ScrollUp => {
                tui.scroll_up();
                **dirty = true;
            }
            TuiAction::ScrollDown => {
                tui.scroll_down();
                **dirty = true;
            }
            TuiAction::ToggleLastThinking => {
                tui.toggle_last_thinking();
                **dirty = true;
            }
            TuiAction::ExpandAllThinking => {
                tui.expand_all_thinking();
                **dirty = true;
            }
            TuiAction::CollapseAllThinking => {
                tui.collapse_all_thinking();
                **dirty = true;
            }
            TuiAction::Submit => {
                handle_submit(&mut tui, &mut exit, &channel);
                tui.scroll_to_bottom();
                **dirty = true;
            }
            TuiAction::ToggleThinking {
                block_index,
                thinking_index,
            } => {
                let bi = *block_index;
                let ti = *thinking_index;
                if let Some(tb) = tui.blocks.get_mut(bi).and_then(|b| {
                    if let OutputBlock::Response(resp) = b {
                        resp.thinkings.get_mut(ti)
                    } else {
                        None
                    }
                }).filter(|tb| !tb.streaming) {
                    tb.expanded = !tb.expanded;
                }
                tui.selected_block = Some(bi);
                **dirty = true;
            }
            TuiAction::SelectBlock(idx) => {
                let i = *idx;
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
            tui.clear_input();
            exit.write_default();
        }
        "/clear" => {
            tui.clear_output();
            tui.clear_input();
        }
        cmd if cmd.starts_with('/') => {
            let lines = commands::handle_slash_command(cmd);
            tui.push_history(&input);
            tui.push_lines(lines);
            tui.clear_input();
        }
        _ => {
            tui.push_history(&input);
            tui.push_line(Line::from(format!("> {input}")).dark_gray());
            tui.clear_input();
            match channel.sender.send(input) {
                Ok(_) => {}
                Err(e) => {
                    let err_line = Line::from(format!("❌ Failed to send input: {e}")).red();
                    tui.push_line(err_line);
                }
            }
        }
    }
}
