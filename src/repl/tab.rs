//! Tab-key completion loop and large-candidate confirm flow.
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.

use {
    super::{
        completion::{
            common_prefix, completions, expand_session_command_tab, session_path_char_completion,
            session_path_inside_subdirectory, session_path_tab_shows_candidate_list,
            session_path_typed_dotfile_prefix, token_at_bounds, token_bounds, token_prefix,
        },
        terminal::{
            apply_token_replacement, redraw_input_line, sync_inline_hint,
            write_repl_candidate_list, write_repl_candidate_list_truncated,
        },
    },
    crate::{agents::AgentConfig, stdout::StdoutMessage},
    bevy::ecs::message::MessageWriter,
};

#[derive(Default)]
pub(super) struct ReplTabLocals {
    pub cycle: Option<TabCycleState>,
    pub list_confirm: Option<TabListConfirm>,
}

#[derive(Debug, Clone)]
pub(super) struct TabCycleState {
    pub snapshot: String,
    pub cursor: usize,
    pub candidates: Vec<String>,
    pub index: usize,
    /// `/load ./` listed once; further Tab without typing does nothing.
    pub session_dir_listed: bool,
}

#[derive(Debug, Clone)]
pub(super) struct TabListConfirm {
    pub candidates: Vec<String>,
}

/// Match rustyline `completion_prompt_limit` (yoyo uses 50).
pub(super) const COMPLETION_PROMPT_LIMIT: usize = 50;

fn tab_list_confirm_prompt(count: usize) -> String {
    format!("Display all {count} possibilities? (y or n)")
}

fn write_tab_list_confirm_prompt(stdout: &mut MessageWriter<StdoutMessage>, count: usize) {
    stdout.write(StdoutMessage::newline());
    // No trailing newline — cursor stays at end of prompt (readline-style).
    stdout.write(StdoutMessage::from(tab_list_confirm_prompt(count)));
}

pub(super) fn accept_tab_candidate_list(
    stdout: &mut MessageWriter<StdoutMessage>,
    confirm: TabListConfirm,
    content: &str,
    cursor: usize,
    agent_config: &AgentConfig,
    tab_cycle: &mut Option<TabCycleState>,
    hint_width: &mut usize,
) {
    write_repl_candidate_list(stdout, &confirm.candidates);
    redraw_input_line(stdout, content, cursor, agent_config, hint_width);
    *tab_cycle = Some(TabCycleState {
        snapshot: content.to_string(),
        cursor,
        candidates: confirm.candidates,
        index: 0,
        session_dir_listed: false,
    });
}

pub(super) fn handle_tab_completion(
    content: &mut String,
    cursor: &mut usize,
    stdout: &mut MessageWriter<StdoutMessage>,
    agent_config: &AgentConfig,
    tab: &mut ReplTabLocals,
    hint_width: &mut usize,
) {
    let tab_cycle = &mut tab.cycle;
    let tab_list_confirm = &mut tab.list_confirm;
    if expand_session_command_tab(content, cursor) {
        if *hint_width > 0 {
            stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
            *hint_width = 0;
        }
        stdout.write(StdoutMessage::from(" "));
        () = sync_inline_hint(stdout, content, *cursor, agent_config, hint_width);
        *tab_cycle = None;
        return;
    }

    if let Some(state) = tab_cycle.as_ref() {
        if state.session_dir_listed && state.snapshot == *content && state.cursor == *cursor {
            *tab_cycle = None;
            return;
        }
    }

    if let Some(state) = tab_cycle.as_ref() {
        if state.snapshot == *content
            && state.cursor == *cursor
            && state.candidates.len() > 1
            && !session_path_char_completion(content, *cursor)
        {
            let next_index = (state.index + 1) % state.candidates.len();
            let replacement = state.candidates[next_index].clone();
            let (start_char, end_char) = token_bounds(content, *cursor);
            () = apply_token_replacement(
                content,
                cursor,
                stdout,
                start_char,
                end_char,
                &replacement,
                agent_config,
                hint_width,
            );
            *tab_cycle = Some(TabCycleState {
                snapshot: content.clone(),
                cursor: *cursor,
                candidates: state.candidates.clone(),
                index: next_index,
                session_dir_listed: false,
            });
            return;
        }
    }

    let candidates = completions(content, *cursor, agent_config);
    if candidates.is_empty() {
        *tab_cycle = None;
        return;
    }

    let (start_char, end_char) = token_bounds(content, *cursor);
    let prefix = token_prefix(content, *cursor);
    let token = token_at_bounds(content, start_char, end_char);
    let path_mode = session_path_char_completion(content, *cursor);
    let mut show_path_list = session_path_tab_shows_candidate_list(content, *cursor);

    if candidates.len() == 1 {
        if candidates[0] != token {
            apply_token_replacement(
                content,
                cursor,
                stdout,
                start_char,
                end_char,
                &candidates[0],
                agent_config,
                hint_width,
            );
        }
        *tab_cycle = None;
        return;
    }

    if !show_path_list {
        let shared = common_prefix(&candidates);
        if shared.len() > prefix.len() {
            if shared != token {
                apply_token_replacement(
                    content,
                    cursor,
                    stdout,
                    start_char,
                    end_char,
                    &shared,
                    agent_config,
                    hint_width,
                );
            }
            if !path_mode {
                *tab_cycle = Some(TabCycleState {
                    snapshot: content.clone(),
                    cursor: *cursor,
                    candidates,
                    index: 0,
                    session_dir_listed: false,
                });
            } else {
                *tab_cycle = None;
            }
            return;
        }

        if path_mode {
            let ambiguous_path = candidates.len() > 1
                && (session_path_typed_dotfile_prefix(prefix)
                    || session_path_inside_subdirectory(prefix));
            if ambiguous_path {
                show_path_list = true;
            } else {
                *tab_cycle = None;
                return;
            }
        }
    }

    let count = candidates.len();
    if *hint_width > 0 {
        stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
        *hint_width = 0;
    }
    if count > COMPLETION_PROMPT_LIMIT {
        *tab_list_confirm = Some(TabListConfirm { candidates });
        () = write_tab_list_confirm_prompt(stdout, count);
        *tab_cycle = None;
        return;
    }

    if count > COMPLETION_PROMPT_LIMIT {
        () = write_repl_candidate_list_truncated(stdout, &candidates, COMPLETION_PROMPT_LIMIT);
    } else {
        () = write_repl_candidate_list(stdout, &candidates);
    }
    () = redraw_input_line(stdout, content, *cursor, agent_config, hint_width);
    *tab_cycle = Some(TabCycleState {
        snapshot: content.clone(),
        cursor: *cursor,
        candidates,
        index: 0,
        session_dir_listed: show_path_list,
    });
}
