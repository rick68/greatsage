//! Action dispatch system — translates [`TuiAction`] messages into mutations
//! on [`TuiMain`].
//!
//! This is the single place where business logic lives.  Input handlers are
//! kept thin (event → message); this system owns the "what does it mean" layer.
//!
//! [`TuiAction`]: crate::tui::events::TuiAction

#![allow(clippy::collapsible_if)]

use {
    crate::{
        agents::CodingAgentPromptChannel,
        config::AppConfig,
        handle_prompt,
        tui::{
            commands,
            core::{CursorState, OutputBlock, TuiMain, TuiMainFocus},
            events::{RenderNeeded, TuiAction},
        },
    },
    bevy::{
        app::AppExit,
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            message::{MessageReader, MessageWriter},
        },
        state::state::NextState,
    },
    ratatui::{style::Stylize, text::Line},
    std::mem,
    unicode_segmentation::UnicodeSegmentation,
    unicode_width::UnicodeWidthChar,
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
    app_config: Res<AppConfig>,
) {
    for action_msg in actions.read() {
        // Any user interaction resets the idle timer and switches back to Blink mode.
        () = tui.reset_activity();
        () = next_cursor_state.set(CursorState::Blink);

        let action = action_msg;

        // Clear selection on any input/action that isn't related to selection itself
        match action {
            TuiAction::SetSelection { .. } | TuiAction::CopySelection => {}
            _ => {
                if tui.selection.is_some() {
                    tui.selection = None;
                    tui.selection_state = crate::tui::core::SelectionState::Idle;
                    **dirty = true;
                }
            }
        }

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
            TuiAction::Quit => {
                exit.write_default();
            }
            TuiAction::ClearSelection => {
                tui.selection = None;
                tui.selection_state = crate::tui::core::SelectionState::Idle;
                **dirty = true;
            }
            TuiAction::SetSelection {
                start,
                end,
                click_count,
            } => {
                let (s_row, s_col) = *start;
                let (e_row, e_col) = *end;
                let cc = *click_count;

                if cc > 1 {
                    handle_multi_click(&mut tui, (s_row, s_col), (e_row, e_col), cc);
                } else {
                    if tui.selection_state == crate::tui::core::SelectionState::Idle {
                        tui.selection_state = crate::tui::core::SelectionState::Dragging {
                            anchor: (s_row, s_col),
                        };
                    }
                    if let crate::tui::core::SelectionState::Dragging { anchor } =
                        tui.selection_state
                    {
                        tui.selection = Some(crate::tui::core::SelectionRange {
                            start: anchor,
                            end: (e_row, e_col),
                        });
                    }
                }
                **dirty = true;
            }
            TuiAction::CopySelection => {
                if let Some(range) = tui.selection {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let text = extract_selection_text(&tui, range);
                        if !text.is_empty() {
                            let _ = clipboard.set_text(text);
                        }
                    }
                }
                tui.selection = None;
                tui.selection_state = crate::tui::core::SelectionState::Idle;
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
                handle_submit(
                    &mut tui,
                    &mut exit,
                    &channel,
                    app_config.repl_error_handling,
                );
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
    repl_error_handling: bool,
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
            // Validation if REPL error handling is enabled
            if repl_error_handling {
                if let Err(e) = handle_prompt(input.clone(), true) {
                    let err_line = Line::from(format!("❌ Prompt validation error: {e}")).red();
                    () = tui.push_line(err_line);
                    // Discard the prompt, do not send.
                    return;
                }
            }
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

fn handle_multi_click(
    t: &mut crate::tui::core::TuiMain,
    r: (usize, usize),
    _e: (usize, usize),
    ct: u8,
) {
    if ct <= 1 {
        return;
    }
    let (row, col) = r;
    let (al, _) =
        crate::tui::renderer::display_utils::rendered_flat_lines(t, &crate::tui::core::SPINNER);
    let iw = t.output_area.width.saturating_sub(2) as usize;
    if al.is_empty() || iw == 0 {
        return;
    }
    let (wr, _) = crate::tui::renderer::display_utils::hard_wrap_output_lines_with_map(
        &al,
        &vec![None; al.len()],
        iw,
    );
    if let Some(l) = wr.get(row) {
        let mut lt = String::new();
        for s in &l.spans {
            lt.push_str(&s.content);
        }
        if ct == 2 {
            let mut cw = 0;
            let mut ci = Vec::new();
            for (i, ch) in lt.char_indices() {
                let w = ch.width().unwrap_or(1);
                ci.push((i, cw, w));
                cw += w;
            }
            let mut ti = lt.len();
            for (idx, ws, w) in &ci {
                if col >= *ws && col < (*ws + *w) {
                    ti = *idx;
                    break;
                }
            }
            let mut sc = 0;
            let mut ec = cw;
            let ws_b = lt.split_word_bound_indices().collect::<Vec<_>>();
            for i in 0..ws_b.len() {
                let (idx, _) = ws_b[i];
                let ni = if i + 1 < ws_b.len() {
                    ws_b[i + 1].0
                } else {
                    lt.len()
                };
                if ti >= idx && ti < ni {
                    for (c_i, c_ws, c_w) in &ci {
                        if *c_i == idx {
                            sc = *c_ws;
                        }
                        if *c_i < ni {
                            ec = *c_ws + *c_w;
                        }
                    }
                    break;
                }
            }
            t.selection = Some(crate::tui::core::SelectionRange {
                start: (row, sc),
                end: (row, ec),
            });
        } else if ct == 3 {
            let mut tw = 0;
            for s in &l.spans {
                for ch in s.content.chars() {
                    tw += ch.width().unwrap_or(1);
                }
            }
            t.selection = Some(crate::tui::core::SelectionRange {
                start: (row, 0),
                end: (row, tw),
            });
        }
    }
}

fn extract_selection_text(
    t: &crate::tui::core::TuiMain,
    r: crate::tui::core::SelectionRange,
) -> String {
    let (mut sr, mut sc) = r.start;
    let (mut er, mut ec) = r.end;
    if sr > er || (sr == er && sc > ec) {
        () = mem::swap(&mut sr, &mut er);
        () = mem::swap(&mut sc, &mut ec);
    }
    let (al, _) =
        crate::tui::renderer::display_utils::rendered_flat_lines(t, &crate::tui::core::SPINNER);
    let iw = t.output_area.width.saturating_sub(2) as usize;
    if al.is_empty() || iw == 0 {
        return String::new();
    }
    let (wr, _) = crate::tui::renderer::display_utils::hard_wrap_output_lines_with_map(
        &al,
        &vec![None; al.len()],
        iw,
    );
    let mut res = String::new();
    for row in sr..=er {
        if let Some(l) = wr.get(row) {
            let mut lt = String::new();
            for s in &l.spans {
                () = lt.push_str(&s.content);
            }
            let stc = if row == sr { sc } else { 0 };
            let enc = if row == er { ec } else { 10000 };
            let mut cw = 0;
            let mut rc = String::new();
            for ch in lt.chars() {
                let w = ch.width().unwrap_or(1);
                if cw >= stc && cw < enc {
                    () = rc.push(ch);
                }
                cw += w;
            }
            if !res.is_empty() {
                () = res.push('\n');
            }
            () = res.push_str(&rc);
        }
    }
    res
}
