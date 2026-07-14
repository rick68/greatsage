//! Keyboard input via `bevy_ratatui::event::KeyMessage`.

use {
    super::{
        commands::is_ui_quit_line,
        state::{TuiFocus, TuiState},
    },
    crate::{
        agents::{AgentConfig, CodingAgent, CodingAgentPromptChannel},
        cli::Cli,
        config::Config,
        repl::{DispatchResult, dispatch_slash_command, session_state::ReplSessionState},
        session::SessionRuntimeStatus,
    },
    bevy::{
        app::AppExit,
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageReader, MessageWriter},
            system::Query,
        },
    },
    bevy_ratatui::{
        crossterm::event::{KeyCode, KeyEventKind, KeyModifiers},
        event::KeyMessage,
    },
    bevy_tokio_tasks::TokioTasksRuntime,
};

/// Finish `/run` / `!` when the worker completes (shared shell path with line REPL).
pub fn poll_shell_system(mut session: ResMut<ReplSessionState>, mut state: ResMut<TuiState>) {
    let Some(handle) = session.active_shell.as_ref() else {
        return;
    };
    let Some(result) = crate::repl::shell_run::try_recv_shell_result(handle) else {
        return;
    };
    session.active_shell = None;
    if result.success {
        session.last_failed_run = None;
    } else {
        session.last_failed_run = Some(result.clone());
    }
    let (output, detail) = crate::repl::shell_run::format_run_output_lines(&result);
    for line in output.into_iter().chain(detail) {
        () = state.push_status(line);
    }
}

pub fn input_system(
    mut keys: MessageReader<KeyMessage>,
    mut state: ResMut<TuiState>,
    mut exit: MessageWriter<AppExit>,
    prompt_channel: Res<CodingAgentPromptChannel>,
    mut agent_config: ResMut<AgentConfig>,
    mut session: ResMut<ReplSessionState>,
    config: Res<Config>,
    cli: Res<Cli>,
    coding_agent: Option<Res<CodingAgent>>,
    tokio_runtime: Res<TokioTasksRuntime>,
    runtime_status: Query<&SessionRuntimeStatus>,
) {
    for message in keys.read() {
        if message.kind != KeyEventKind::Press && message.kind != KeyEventKind::Repeat {
            continue;
        }

        if message.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(message.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            // Second consecutive Ctrl+C → Leave app.
            if session.ctrl_c_armed {
                if let Some(handle) = session.active_shell.as_mut() {
                    crate::repl::shell_run::request_shell_interrupt(handle);
                    crate::repl::shell_run::request_shell_interrupt(handle);
                }
                session.active_shell = None;
                session.ctrl_c_armed = false;
                exit.write_default();
                return;
            }

            if let Some(handle) = session.active_shell.as_mut() {
                () = crate::repl::shell_run::request_shell_interrupt(handle);
                session.ctrl_c_armed = true;
                continue;
            }

            let agent_busy = runtime_status.iter().any(|s| s.is_processing());
            if agent_busy {
                if let Some(agent_res) = coding_agent.as_ref() {
                    let agent: crate::agents::CodingAgent = (**agent_res).clone();
                    tokio_runtime.runtime().spawn(async move {
                        let guard = agent.lock().await;
                        guard.abort();
                    });
                }
                session.ctrl_c_armed = true;
                () = state.push_status("^C");
                continue;
            }

            // Idle first press: cancel line + arm
            () = state.clear_prompt();
            session.ctrl_c_armed = true;
            continue;
        }

        if message.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(message.code, KeyCode::Char('d') | KeyCode::Char('D'))
        {
            if let Some(handle) = session.active_shell.as_ref() {
                crate::repl::shell_run::request_shell_stdin_eof(handle);
                continue;
            }
            session.ctrl_c_armed = false;
            exit.write_default();
            return;
        }

        if session.active_shell.is_some() {
            continue;
        }

        // Other keys clear double-Ctrl+C arm
        if session.ctrl_c_armed {
            session.ctrl_c_armed = false;
        }

        match message.code {
            KeyCode::Tab => {
                state.focus = match state.focus {
                    TuiFocus::Prompt => TuiFocus::Scrollback,
                    TuiFocus::Scrollback => TuiFocus::Prompt,
                };
            }
            KeyCode::Esc => {
                state.focus = TuiFocus::Prompt;
            }
            KeyCode::PageUp if state.focus == TuiFocus::Scrollback => {
                state.scroll_from_bottom = state.scroll_from_bottom.saturating_add(5);
            }
            KeyCode::PageDown if state.focus == TuiFocus::Scrollback => {
                state.scroll_from_bottom = state.scroll_from_bottom.saturating_sub(5);
            }
            KeyCode::Char(c) if state.focus == TuiFocus::Prompt => {
                let idx = state.cursor.min(state.prompt.len());
                () = state.prompt.insert(idx, c);
                state.cursor = idx + c.len_utf8();
            }
            KeyCode::Backspace if state.focus == TuiFocus::Prompt => {
                if state.cursor > 0 {
                    let idx = state.cursor;
                    let prev = state.prompt[..idx]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    () = state.prompt.replace_range(prev..idx, "");
                    state.cursor = prev;
                }
            }
            KeyCode::Left if state.focus == TuiFocus::Prompt => {
                if state.cursor > 0 {
                    let prev = state.prompt[..state.cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    state.cursor = prev;
                }
            }
            KeyCode::Right if state.focus == TuiFocus::Prompt => {
                if state.cursor < state.prompt.len() {
                    let next = state.prompt[state.cursor..]
                        .chars()
                        .next()
                        .map(|c| state.cursor + c.len_utf8())
                        .unwrap_or(state.prompt.len());
                    state.cursor = next;
                }
            }
            KeyCode::Enter if state.focus == TuiFocus::Prompt => {
                let line = state.prompt.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                () = state.clear_prompt();
                if is_ui_quit_line(&line) {
                    exit.write_default();
                    return;
                }
                if line.starts_with('/') {
                    let session_processing = runtime_status.iter().any(|s| s.is_processing());
                    () = handle_slash(
                        &line,
                        &mut state,
                        &mut agent_config,
                        &mut session,
                        &config,
                        &cli,
                        coding_agent.as_deref(),
                        tokio_runtime.runtime(),
                        session_processing,
                        &mut exit,
                        &prompt_channel,
                    );
                } else {
                    session.ctrl_c_armed = false;
                    () = prompt_channel.send_prompt(line);
                    () = state.push_status("prompt sent");
                }
            }
            _ => {}
        }
    }
}

fn handle_slash(
    line: &str,
    state: &mut TuiState,
    agent_config: &mut AgentConfig,
    session: &mut ReplSessionState,
    config: &Config,
    cli: &Cli,
    coding_agent: Option<&CodingAgent>,
    runtime: &tokio::runtime::Runtime,
    session_processing: bool,
    exit: &mut MessageWriter<AppExit>,
    prompt_channel: &CodingAgentPromptChannel,
) {
    match dispatch_slash_command(
        line,
        agent_config,
        session,
        config,
        None,
        coding_agent,
        runtime,
        None,
        cli.bare,
        session_processing,
    ) {
        DispatchResult::Exit => {
            let _ = exit.write_default();
        }
        DispatchResult::Handled {
            output,
            detail,
            redraw_prompt: _,
            reinstall,
        } => {
            for line in output.into_iter().chain(detail) {
                () = state.push_status(line);
            }
            if reinstall.is_some() {
                () = state.push_status(
                    "(reinstall requested — use line REPL for full agent reinstall in foundation)",
                );
            }
        }
        DispatchResult::ResendPrompt { prompt, hint } => {
            () = state.push_status(hint);
            session.ctrl_c_armed = false;
            () = prompt_channel.send_prompt(prompt);
        }
        DispatchResult::AgentOp(inv) => {
            for line in inv.preamble {
                () = state.push_status(line);
            }
            () = state.push_status(
                "(agent file ops simplified in TUI foundation — use line REPL for /save /load /jump)",
            );
        }
        DispatchResult::AwaitClearConfirm { prompt } => {
            () = state.push_status(prompt);
            () = state
                .push_status("clear confirm: use line REPL for multi-step confirm in foundation");
        }
        DispatchResult::Unknown => {
            () = state.push_status(format!("unknown command: {line}"));
        }
    }
}
