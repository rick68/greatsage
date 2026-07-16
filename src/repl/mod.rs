//! Bevy REPL plugin: stdin loop, slash dispatch, tab completion, async agent ops.
//!
//! Items are ordered for bottom-up reading: the file entry ([`repl_plugin`]) is last.
//! Each function sits directly above its callers; local callees are placed immediately
//! above the function that references them, in source appearance order. Reuse earlier
//! definitions when a callee is already defined above.

mod commands_help;
mod commands_hooks;
mod commands_info;
mod commands_lifecycle;
mod commands_memory;
mod commands_project;
mod commands_session;
mod commands_session_nav;
mod commands_shell;
mod completion;
mod context_display;
mod cost;
mod dispatch;
/// Re-export for TUI slash autocomplete (same catalog engine as line REPL).
pub(crate) use completion::{
    apply_replacement, common_prefix, completions, inline_hint, token_bounds, token_prefix,
};
/// Re-export for `tui` slash forwarding (same dispatch as line REPL).
pub(crate) use dispatch::{DispatchResult, dispatch_slash_command};
pub(crate) mod help_data;
mod history;
mod model_cmd;
pub(crate) mod model_id;
/// Static rate book for `/cost` / `/model info`; also fills `ModelConfig.cost` at agent install.
pub(crate) mod native_pricing;
mod output;
mod path_display;
mod route;
mod session_dashboard;
mod session_nav;
pub(crate) mod session_ops;
pub(crate) mod session_state;
pub(crate) mod shell_bg;
pub(crate) mod shell_run;
pub(crate) mod startup_hints;

mod suggest;
mod tab;
mod terminal;

use {
    crate::{
        agents::{
            AgentConfig, CodingAgent, CodingAgentPromptChannel, CodingAgentTask,
            install_coding_agent, prepare_coding_agent_preserving_messages,
        },
        cli::Cli,
        config::Config,
        project_context::assemble_system_prompt,
        session::{
            SessionContextStats, SessionId, SessionLifetimeUsage, SessionManager, SessionMeta,
            SessionRuntimeStatus, TurnEntity, TurnSummary,
            context_stats::sync_context_stats_on_world,
        },
        stdin::StdinKeyMessage,
        stdout::{ExternPromptSubmitted, StdoutMessage, prompt_symbol, prompt_symbol_inline},
        tokio::AppCancelToken,
    },
    bevy::{
        app::{App, AppExit, PostUpdate, PreUpdate, Startup, Update},
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageReader, MessageWriter},
            query::With,
            schedule::IntoScheduleConfigs,
            system::{Commands, Local, Query, SystemParam},
        },
    },
    bevy_ratatui::crossterm::{
        self,
        event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    },
    bevy_tokio_tasks::TokioTasksRuntime,
    dispatch::{AgentOp, AgentOpInvocation, build_unknown_slash_feedback, command_name_and_args},
    history::{DEFAULT_MAX_ENTRIES, ReplInputHistory, persist_repl_history},
    output::ReplOutputChannel,
    route::{CommandRoute, route_command},
    session_dashboard::{
        SessionContextStatsFields, SessionMetaFields, TurnUsageRow, build_snapshot,
    },
    session_ops::{
        abort_agent_best_effort, agent_message_stats, block_on_session, compact_agent_with_keep,
        last_user_prompt_from_messages, load_agent_from_bookmark, load_agent_from_file,
        save_messages, try_auto_save_session,
    },
    session_state::ReplSessionState,
    std::{env, path::PathBuf},
    tab::{ReplTabLocals, TabListConfirm, accept_tab_candidate_list, handle_tab_completion},
    terminal::{
        erase_ahead_echo, redraw_input_line, replace_input_line_in_place, sync_inline_hint,
        write_quit_farewell_if_enabled, write_repl_handled_output, write_repl_response,
        write_repl_response_lines, write_repl_shell_stream_line, write_unknown_slash_feedback,
    },
    unicode_width::UnicodeWidthChar,
};

fn setup(_app_cancel: Res<AppCancelToken>, _tokio_runtime: ResMut<TokioTasksRuntime>) {
    () = crossterm::terminal::enable_raw_mode().expect("Failed to enable raw mode");
}

fn print_system_prompt(
    config: Res<Config>,
    cli: Res<Cli>,
    mut stdout: MessageWriter<StdoutMessage>,
    mut exit: MessageWriter<AppExit>,
) {
    let base_prompt = config.get_system_prompt();
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (system_prompt, _) = assemble_system_prompt(&base_prompt, &cwd, cli.bare);
    stdout.write(StdoutMessage::from(
        system_prompt.trim_end_matches([' ', '\t', '\n']),
    ));
    exit.write_default();
}

fn persist_session_on_exit(coding_agent: Option<&CodingAgent>, runtime: &tokio::runtime::Runtime) {
    let Some(agent) = coding_agent else {
        return;
    };
    // Prefer abort before save so we are not stuck behind an in-flight turn's lock.
    () = abort_agent_best_effort(runtime, agent);
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Err(err) = block_on_session(runtime, try_auto_save_session(agent, &cwd)) {
        eprintln!("auto-save error: {err}");
    }
}

/// Ctrl+C: first press = interrupt/abort/cancel-line + arm; second = Leave app.
/// Idle first-press cancel-line needs input buffer → [`read_stdin_stream`].
fn ctrl_c(
    mut commands: Commands,
    mut stdin_key_message: MessageReader<StdinKeyMessage>,
    mut exit: MessageWriter<AppExit>,
    mut stdout: MessageWriter<StdoutMessage>,
    mut history: ResMut<ReplInputHistory>,
    mut session_state: ResMut<ReplSessionState>,
    cli: Res<Cli>,
    coding_agent: Option<Res<CodingAgent>>,
    mut agent_task: Option<ResMut<CodingAgentTask>>,
    tokio_runtime: Res<TokioTasksRuntime>,
) {
    for StdinKeyMessage(KeyEvent {
        code,
        modifiers,
        kind,
        ..
    }) in stdin_key_message.read()
    {
        if *kind != KeyEventKind::Press && *kind != KeyEventKind::Repeat {
            continue;
        }
        if *code != KeyCode::Char('c') || !modifiers.contains(KeyModifiers::CONTROL) {
            continue;
        }

        // Second consecutive Ctrl+C → Leave app (always).
        if session_state.ctrl_c_armed {
            leave_repl_app(
                &mut commands,
                &mut exit,
                &mut stdout,
                history.as_mut(),
                cli.as_ref(),
                coding_agent.as_deref(),
                &tokio_runtime,
                Some(session_state.as_mut()),
            );
            continue;
        }

        // First press: shell → interrupt + white ^C + arm (exit footer comes when child ends)
        if let Some(handle) = session_state.active_shell.as_mut() {
            shell_run::request_shell_interrupt(handle);
            session_state.ctrl_c_armed = true;
            stdout.write(StdoutMessage::from(
                <&str as colored::Colorize>::bright_white("^C").to_string(),
            ));
            stdout.write(StdoutMessage::newline());
            continue;
        }

        // First press: agent mid-turn → abort + ^C + arm
        if agent_task.is_some() {
            if let Some(task) = agent_task.as_mut() {
                task.suppress_stream = true;
            }
            if let Some(agent_res) = coding_agent.as_ref() {
                let agent: CodingAgent = (**agent_res).clone();
                tokio_runtime.runtime().spawn(async move {
                    let guard = agent.lock().await;
                    guard.abort();
                });
            }
            session_state.ctrl_c_armed = true;
            stdout.write(StdoutMessage::from(
                <&str as colored::Colorize>::bright_white("^C").to_string(),
            ));
            stdout.write(StdoutMessage::newline());
            continue;
        }

        // Idle first press: cancel line in read_stdin_stream (sets arm there too).
    }
}

/// Shared Leave-app path (Ctrl+D idle, or second Ctrl+C).
/// Idempotent: `ctrl_c` and `read_stdin_stream` may both see the same key.
fn leave_repl_app(
    commands: &mut Commands,
    exit: &mut MessageWriter<AppExit>,
    stdout: &mut MessageWriter<StdoutMessage>,
    history: &mut ReplInputHistory,
    cli: &Cli,
    coding_agent: Option<&CodingAgent>,
    tokio_runtime: &TokioTasksRuntime,
    session_state: Option<&mut ReplSessionState>,
) {
    if let Some(state) = session_state {
        if state.leaving {
            return;
        }
        state.leaving = true;
        if let Some(handle) = state.active_shell.as_mut() {
            // Ensure child tree dies on way out.
            shell_run::request_shell_interrupt(handle);
            shell_run::request_shell_interrupt(handle); // hard if soft already sent
        }
        state.active_shell = None;
        state.ctrl_c_armed = false;
    }
    () = persist_repl_history(history, &crate::config_paths::repl_history_path());
    // Mark leave before auto-save so a second AppExit path can skip duplicate work.
    () = commands.remove_resource::<CodingAgentTask>();
    () = persist_session_on_exit(coding_agent, tokio_runtime.runtime());
    () = write_quit_farewell_if_enabled(stdout, cli);
    exit.write_default();
}

/// Live-stream body + finish footer for in-flight `/run` / `!` (yoyo line stream).
fn poll_active_shell_run(
    mut session_state: ResMut<ReplSessionState>,
    mut stdout: MessageWriter<StdoutMessage>,
) {
    let Some(handle) = session_state.active_shell.as_mut() else {
        return;
    };

    // Drain live lines every frame while the child is still running.
    while let Some(live) = shell_run::try_recv_shell_live(handle) {
        handle.body_streamed = true;
        let line = shell_run::format_live_stream_line(&live);
        () = write_repl_shell_stream_line(&mut stdout, &line);
    }

    let Some(result) = shell_run::try_recv_shell_result(handle) else {
        return;
    };
    // Drain any lines that arrived in the same tick as the result.
    while let Some(live) = shell_run::try_recv_shell_live(handle) {
        handle.body_streamed = true;
        let line = shell_run::format_live_stream_line(&live);
        () = write_repl_shell_stream_line(&mut stdout, &line);
    }
    let body_streamed = handle.body_streamed;
    session_state.active_shell = None;
    if result.success {
        session_state.last_failed_run = None;
    } else {
        session_state.last_failed_run = Some(result.clone());
    }
    let (output, detail) = shell_run::format_run_output_lines_ex(&result, body_streamed);
    () = write_repl_handled_output(&mut stdout, &output, &detail);
    stdout.write(StdoutMessage::from(prompt_symbol()));
}

fn drain_repl_output(
    channel: &ReplOutputChannel,
    stdout: &mut MessageWriter<StdoutMessage>,
) -> bool {
    let mut drained = false;
    while let Ok(line) = channel.receiver.try_recv() {
        () = write_repl_response(stdout, &line);
        drained = true;
    }
    drained
}

fn echo_extern_submitted_prompt(
    stdout: &mut MessageWriter<StdoutMessage>,
    input: &mut ReplInputLocals,
    prompt: &str,
    history: &mut ReplInputHistory,
    session_state: &mut ReplSessionState,
) {
    () = input.clear_tab_state();
    () = input.clear_inline_hint(stdout);
    stdout.write(StdoutMessage::from("\r"));
    stdout.write(StdoutMessage::from(format!(
        "{}{}",
        prompt_symbol_inline(),
        prompt
    )));
    stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
    stdout.write(StdoutMessage::newline());
    () = history.push_submitted(prompt);
    session_state.last_user_prompt = Some(prompt.to_owned());
    () = input.clear_line();
}

fn spawn_agent_reinstall(tokio_runtime: &mut TokioTasksRuntime, agent_config: AgentConfig) {
    let model = agent_config.model.clone();
    let provider = agent_config.provider.to_string();
    tokio_runtime.spawn_background_task(|mut ctx| async move {
        let coding_agent = CodingAgent::new_with_agent_config(&agent_config).await;
        ctx.run_on_main_thread(move |main_ctx| {
            () = install_coding_agent(main_ctx.world, coding_agent, model, provider);
        })
        .await;
    });
}

fn spawn_agent_op(
    tokio_runtime: &mut TokioTasksRuntime,
    op: AgentOp,
    output_channel: &ReplOutputChannel,
    coding_agent: Option<CodingAgent>,
) {
    let sender = output_channel.sender.clone();
    tokio_runtime.spawn_background_task(|mut ctx| async move {
        let result = match op {
            AgentOp::Save { path } => match coding_agent {
                Some(agent) => save_messages(&agent, &path).await,
                None => Err(String::from("No active agent.")),
            },
            AgentOp::Load { path, config } => match load_agent_from_file(&config, &path).await {
                Ok((new_agent, message, messages)) => {
                    let model = config.model.clone();
                    let provider = config.provider.to_string();
                    let context_max = u64::from(new_agent.context_window());
                    let last_prompt = last_user_prompt_from_messages(&messages);
                    ctx.run_on_main_thread(move |main| {
                        () = install_coding_agent(main.world, new_agent, model, provider);
                        if let Some(agent) = main.world.get_resource::<CodingAgent>() {
                            let session_id = agent.session_id();
                            sync_context_stats_on_world(
                                main.world,
                                session_id,
                                &messages,
                                context_max,
                            );
                        }
                        if let Some(mut state) = main.world.get_resource_mut::<ReplSessionState>() {
                            state.last_user_prompt = last_prompt;
                        }
                    })
                    .await;
                    Ok(message)
                }
                Err(err) => Err(err),
            },
            AgentOp::Jump { json, config, name } => {
                match load_agent_from_bookmark(&config, &json, &name).await {
                    Ok((new_agent, message, messages)) => {
                        let model = config.model.clone();
                        let provider = config.provider.to_string();
                        let context_max = u64::from(new_agent.context_window());
                        let last_prompt = last_user_prompt_from_messages(&messages);
                        ctx.run_on_main_thread(move |main| {
                            () = install_coding_agent(main.world, new_agent, model, provider);
                            if let Some(agent) = main.world.get_resource::<CodingAgent>() {
                                let session_id = agent.session_id();
                                sync_context_stats_on_world(
                                    main.world,
                                    session_id,
                                    &messages,
                                    context_max,
                                );
                            }
                            if let Some(mut state) =
                                main.world.get_resource_mut::<ReplSessionState>()
                            {
                                state.last_user_prompt = last_prompt;
                            }
                        })
                        .await;
                        Ok(message)
                    }
                    Err(err) => Err(err),
                }
            }
            AgentOp::Compact { keep_recent } => match coding_agent {
                Some(agent) => compact_agent_with_keep(&agent, keep_recent).await,
                None => Err(String::from("No active agent.")),
            },
            AgentOp::ReinstallPreserveMessages {
                config,
                success_message,
            } => match coding_agent {
                None => Err(String::from("No active agent.")),
                Some(agent) => {
                    let model = config.model.clone();
                    let provider = config.provider.to_string();
                    match prepare_coding_agent_preserving_messages(&agent, &config, success_message)
                        .await
                    {
                        Ok((new_agent, message)) => {
                            ctx.run_on_main_thread(move |main| {
                                () = install_coding_agent(main.world, new_agent, model, provider);
                            })
                            .await;
                            Ok(message)
                        }
                        Err(err) => Err(err),
                    }
                }
            },
        };

        let line = match result {
            Ok(msg) => msg,
            Err(err) => format!("Error: {err}"),
        };
        let _ = sender.send(line);
    });
}

#[derive(Default)]
struct ReplInputLocals {
    cursor: usize,
    content: String,
    tab: ReplTabLocals,
    hint_width: usize,
    /// Previous frame had an in-flight [`CodingAgentTask`] (prompt streaming).
    was_agent_busy: bool,
    /// Display columns of type-ahead text echoed inline after streaming output.
    ahead_echo_width: usize,
    /// Formal `> ` prompt line is active (idle editing / post-run restore).
    on_prompt_line: bool,
}

impl ReplInputLocals {
    fn reset_line(&mut self) {
        self.cursor = 0;
        () = self.content.clear();
        self.hint_width = 0;
        self.ahead_echo_width = 0;
        self.on_prompt_line = false;
    }

    fn clear_line(&mut self) {
        self.cursor = 0;
        () = self.content.clear();
        self.ahead_echo_width = 0;
        self.on_prompt_line = false;
    }

    fn text_display_width(text: &str) -> usize {
        text.chars()
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
            .sum()
    }

    fn char_display_width(c: char) -> usize {
        UnicodeWidthChar::width(c).unwrap_or(0)
    }

    fn clear_tab_state(&mut self) {
        self.tab.cycle = None;
        self.tab.list_confirm = None;
    }

    fn clear_inline_hint(&mut self, stdout: &mut MessageWriter<StdoutMessage>) {
        if self.hint_width > 0 {
            stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
            self.hint_width = 0;
        }
    }

    fn redraw_line(
        &mut self,
        stdout: &mut MessageWriter<StdoutMessage>,
        agent_config: &AgentConfig,
    ) {
        redraw_input_line(
            stdout,
            &self.content,
            self.cursor,
            agent_config,
            &mut self.hint_width,
        );
    }

    fn accept_tab_list(
        &mut self,
        stdout: &mut MessageWriter<StdoutMessage>,
        confirm: TabListConfirm,
        agent_config: &AgentConfig,
    ) {
        () = accept_tab_candidate_list(
            stdout,
            confirm,
            &self.content,
            self.cursor,
            agent_config,
            &mut self.tab.cycle,
            &mut self.hint_width,
        );
        self.tab.list_confirm = None;
    }

    fn handle_tab(
        &mut self,
        stdout: &mut MessageWriter<StdoutMessage>,
        agent_config: &AgentConfig,
    ) {
        () = handle_tab_completion(
            &mut self.content,
            &mut self.cursor,
            stdout,
            agent_config,
            &mut self.tab,
            &mut self.hint_width,
        );
    }

    fn sync_hint(&mut self, stdout: &mut MessageWriter<StdoutMessage>, agent_config: &AgentConfig) {
        () = sync_inline_hint(
            stdout,
            &self.content,
            self.cursor,
            agent_config,
            &mut self.hint_width,
        );
    }

    fn apply_history_line(
        &mut self,
        line: String,
        stdout: &mut MessageWriter<StdoutMessage>,
        agent_config: &AgentConfig,
    ) {
        () = self.clear_tab_state();
        self.content = line;
        self.cursor = self.content.chars().count();
        self.on_prompt_line = true;
        () = replace_input_line_in_place(
            stdout,
            &self.content,
            self.cursor,
            agent_config,
            &mut self.hint_width,
        );
    }

    fn replace_input_on_line(
        &mut self,
        stdout: &mut MessageWriter<StdoutMessage>,
        agent_config: &AgentConfig,
    ) {
        () = replace_input_line_in_place(
            stdout,
            &self.content,
            self.cursor,
            agent_config,
            &mut self.hint_width,
        );
    }

    /// Echo one character inline at the current output position (agent busy, append at end).
    fn echo_ahead_char(&mut self, stdout: &mut MessageWriter<StdoutMessage>, c: char) {
        if self.cursor != self.content.chars().count() {
            self.sync_ahead_echo_full(stdout);
            return;
        }
        let mut buf = [0_u8; 4];
        let bytes = c.encode_utf8(&mut buf);
        stdout.write(StdoutMessage::from(bytes));
        self.ahead_echo_width += Self::char_display_width(c);
    }

    /// Rewrite inline type-ahead after history recall or mid-line edit during agent run.
    fn sync_ahead_echo_full(&mut self, stdout: &mut MessageWriter<StdoutMessage>) {
        if self.ahead_echo_width > 0 {
            () = erase_ahead_echo(stdout, self.ahead_echo_width);
            self.ahead_echo_width = 0;
        }
        if self.content.is_empty() {
            return;
        }
        stdout.write(StdoutMessage::from(self.content.as_str()));
        self.ahead_echo_width = Self::text_display_width(&self.content);
    }

    fn erase_ahead_char(&mut self, stdout: &mut MessageWriter<StdoutMessage>, width: usize) {
        if width == 0 {
            return;
        }
        () = erase_ahead_echo(stdout, width);
        self.ahead_echo_width = self.ahead_echo_width.saturating_sub(width);
    }

    fn apply_history_silent(&mut self, line: String) {
        () = self.clear_tab_state();
        self.hint_width = 0;
        self.content = line;
        self.cursor = self.content.chars().count();
    }

    fn clear_ahead_echo(&mut self, stdout: &mut MessageWriter<StdoutMessage>) {
        if self.ahead_echo_width > 0 {
            () = erase_ahead_echo(stdout, self.ahead_echo_width);
            self.ahead_echo_width = 0;
        }
    }

    fn insert_char_buffered(&mut self, c: char) {
        () = self.clear_tab_state();
        if self.cursor == self.content.chars().count() {
            () = self.content.push(c);
            self.cursor += 1;
        } else if let Some((byte_idx, _)) = self.content.char_indices().nth(self.cursor) {
            () = self.content.insert(byte_idx, c);
            self.cursor += 1;
        }
    }

    fn backspace_buffered(&mut self) {
        () = self.clear_tab_state();
        let cursor_pos = self.cursor;
        if cursor_pos != 0
            && let Some((byte_idx, _)) = self.content.char_indices().nth(cursor_pos - 1)
        {
            self.content.remove(byte_idx);
            self.cursor = cursor_pos - 1;
        }
    }

    fn move_left_buffered(&mut self) {
        self.clear_tab_state();
        self.hint_width = 0;
        let cursor_pos = self.cursor;
        if cursor_pos != 0 {
            self.cursor = cursor_pos - 1;
        }
    }

    fn move_right_buffered(&mut self) {
        () = self.clear_tab_state();
        if self.cursor < self.content.chars().count() {
            self.cursor += 1;
        }
    }
}

fn restore_prompt_with_input(
    stdout: &mut MessageWriter<StdoutMessage>,
    input: &mut ReplInputLocals,
    agent_config: &AgentConfig,
) {
    if input.ahead_echo_width > 0 {
        input.ahead_echo_width = 0;
    }
    stdout.write(StdoutMessage::from(prompt_symbol()));
    if input.content.is_empty() {
        input.hint_width = 0;
        input.on_prompt_line = false;
    } else {
        input.on_prompt_line = true;
        stdout.write(StdoutMessage::from(input.content.as_str()));
        () = input.sync_hint(stdout, agent_config);
    }
}

fn handle_agent_busy_key(
    code: KeyCode,
    kind: KeyEventKind,
    modifiers: KeyModifiers,
    input: &mut ReplInputLocals,
    history: &mut ReplInputHistory,
    stdout: &mut MessageWriter<StdoutMessage>,
) -> bool {
    if !matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return false;
    }
    match code {
        KeyCode::Up => {
            if let Some(line) = history.recall_up(&input.content) {
                () = input.clear_ahead_echo(stdout);
                () = input.apply_history_silent(line);
            }
        }
        KeyCode::Down => {
            if let Some(line) = history.recall_down() {
                () = input.clear_ahead_echo(stdout);
                () = input.apply_history_silent(line);
            }
        }
        KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
            if history.is_recalling() {
                () = history.cancel_recall();
            }
            let at_end = input.cursor == input.content.chars().count();
            input.insert_char_buffered(c);
            if at_end && input.cursor == input.content.chars().count() {
                () = input.echo_ahead_char(stdout, c);
            } else {
                () = input.sync_ahead_echo_full(stdout);
            }
        }
        KeyCode::Backspace => {
            if history.is_recalling() {
                () = history.cancel_recall();
            }
            let at_end = input.cursor == input.content.chars().count();
            let removed_width = if at_end && input.cursor > 0 {
                input
                    .content
                    .chars()
                    .nth(input.cursor - 1)
                    .map(ReplInputLocals::char_display_width)
            } else {
                None
            };
            input.backspace_buffered();
            if let Some(width) = removed_width {
                () = input.erase_ahead_char(stdout, width);
            } else {
                () = input.sync_ahead_echo_full(stdout);
            }
        }
        KeyCode::Left => {
            if history.is_recalling() {
                () = history.cancel_recall();
            }
            () = input.move_left_buffered();
        }
        KeyCode::Right => {
            if history.is_recalling() {
                () = history.cancel_recall();
            }
            () = input.move_right_buffered();
        }
        _ => return false,
    }
    true
}

fn history_keys_allowed(session_state: &ReplSessionState, input: &ReplInputLocals) -> bool {
    !session_state.pending_clear_confirm && input.tab.list_confirm.is_none()
}

#[derive(SystemParam)]
struct ReplEcsDashboard<'w, 's> {
    turns: Query<'w, 's, (&'static SessionId, &'static TurnSummary), With<TurnEntity>>,
    session_meta: Query<'w, 's, &'static SessionMeta>,
    context_stats: Query<'w, 's, &'static SessionContextStats>,
    runtime_status: Query<'w, 's, &'static SessionRuntimeStatus>,
    session_manager: Res<'w, SessionManager>,
    lifetime_usage: Res<'w, SessionLifetimeUsage>,
}

impl ReplEcsDashboard<'_, '_> {
    fn turn_rows(&self, session_id: SessionId) -> Vec<TurnUsageRow> {
        self.turns
            .iter()
            .filter(|(sid, _)| **sid == session_id)
            .map(|(_, summary)| TurnUsageRow {
                input_tokens: summary.input_tokens,
                output_tokens: summary.output_tokens,
                cache_read_tokens: summary.cache_read_tokens,
                cache_write_tokens: summary.cache_write_tokens,
            })
            .collect()
    }

    fn meta_fields(&self, session_id: SessionId) -> Option<SessionMetaFields> {
        let root = self.session_manager.root_entity(session_id)?;
        let meta = self.session_meta.get(root).ok()?;
        Some(SessionMetaFields {
            started_at_ms: meta.started_at_ms,
            cwd: meta.cwd.clone(),
        })
    }

    fn context_stats_fields(&self, session_id: SessionId) -> Option<SessionContextStatsFields> {
        let root = self.session_manager.root_entity(session_id)?;
        let stats = self.context_stats.get(root).ok()?;
        Some(SessionContextStatsFields {
            message_count: stats.message_count,
            context_used: stats.context_used,
            context_max: stats.context_max,
        })
    }

    fn is_processing(&self, session_id: SessionId) -> bool {
        let Some(root) = self.session_manager.root_entity(session_id) else {
            return false;
        };
        self.runtime_status
            .get(root)
            .ok()
            .is_some_and(SessionRuntimeStatus::is_processing)
    }
}

fn echo_extern_prompt_submissions(
    mut extern_prompts: MessageReader<ExternPromptSubmitted>,
    mut input: Local<ReplInputLocals>,
    mut stdout: MessageWriter<StdoutMessage>,
    mut history: ResMut<ReplInputHistory>,
    mut session_state: ResMut<ReplSessionState>,
) {
    for ExternPromptSubmitted(prompt) in extern_prompts.read() {
        () = echo_extern_submitted_prompt(
            &mut stdout,
            &mut input,
            &prompt,
            &mut history,
            &mut session_state,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn read_stdin_stream(
    mut stdin_key_messages: MessageReader<StdinKeyMessage>,
    mut input: Local<ReplInputLocals>,
    mut stdout: MessageWriter<StdoutMessage>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
    prompt_channel: Res<CodingAgentPromptChannel>,
    config: Res<Config>,
    mut agent_config: ResMut<AgentConfig>,
    mut session_state: ResMut<ReplSessionState>,
    output_channel: Res<ReplOutputChannel>,
    coding_agent: Option<Res<CodingAgent>>,
    mut tokio_runtime: ResMut<TokioTasksRuntime>,
    mut history: ResMut<ReplInputHistory>,
    agent_task: Option<Res<CodingAgentTask>>,
    cli: Res<Cli>,
    ecs_dashboard: ReplEcsDashboard,
) {
    let agent_busy = agent_task.is_some();
    // Do not redraw `>` while leaving (double Ctrl+C / Ctrl+D).
    if input.was_agent_busy && !agent_busy && !session_state.leaving {
        () = restore_prompt_with_input(&mut stdout, &mut input, agent_config.as_ref());
    }
    input.was_agent_busy = agent_busy;

    if !session_state.leaving && drain_repl_output(&output_channel, &mut stdout) {
        stdout.write(StdoutMessage::from(prompt_symbol()));
        if input.content.is_empty() {
            () = input.reset_line();
        } else {
            input.cursor = input.content.chars().count();
            input.on_prompt_line = true;
            () = input.replace_input_on_line(&mut stdout, agent_config.as_ref());
        }
    }

    for StdinKeyMessage(KeyEvent {
        code,
        modifiers,
        kind,
        ..
    }) in stdin_key_messages.read()
    {
        // Already leaving: ignore further keys (no prompt redraw).
        if session_state.leaving {
            continue;
        }

        // ── Ctrl+C / Ctrl+D by state ──
        if (*kind == KeyEventKind::Press || *kind == KeyEventKind::Repeat)
            && *code == KeyCode::Char('c')
            && modifiers.contains(KeyModifiers::CONTROL)
        {
            // Shell / agent first press is owned by `ctrl_c` (sets `ctrl_c_armed` this
            // frame). Must not treat that same key as a second press here, or we Leave
            // app on a single Ctrl+C during LLM response.
            if session_state.active_shell.is_some() || agent_task.is_some() {
                continue;
            }
            // Idle / post-agent: second Ctrl+C while armed → Leave app.
            if session_state.ctrl_c_armed {
                leave_repl_app(
                    &mut commands,
                    &mut exit,
                    &mut stdout,
                    history.as_mut(),
                    cli.as_ref(),
                    coding_agent.as_deref(),
                    tokio_runtime.as_ref(),
                    Some(session_state.as_mut()),
                );
                return;
            }
            // Idle first press: cancel line + arm (second Ctrl+C leaves).
            () = input.clear_tab_state();
            () = input.clear_inline_hint(&mut stdout);
            stdout.write(StdoutMessage::newline());
            stdout.write(StdoutMessage::from(prompt_symbol()));
            () = input.clear_line();
            session_state.ctrl_c_armed = true;
            continue;
        }

        if *kind == KeyEventKind::Press
            && *code == KeyCode::Char('d')
            && modifiers.contains(KeyModifiers::CONTROL)
        {
            if session_state.active_shell.is_some() {
                if let Some(handle) = session_state.active_shell.as_ref() {
                    shell_run::request_shell_stdin_eof(handle);
                }
                continue;
            }
            // Idle: Leave app (yoyo ReadlineError::Eof)
            () = leave_repl_app(
                &mut commands,
                &mut exit,
                &mut stdout,
                history.as_mut(),
                cli.as_ref(),
                coding_agent.as_deref(),
                tokio_runtime.as_ref(),
                Some(session_state.as_mut()),
            );
            return;
        }

        // Any other key clears the double-Ctrl+C arm.
        if session_state.ctrl_c_armed
            && *kind == KeyEventKind::Press
            && !(*code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL))
        {
            session_state.ctrl_c_armed = false;
        }

        // Active shell: ignore other line editing / Enter until run finishes.
        if session_state.active_shell.is_some() {
            continue;
        }

        if agent_task.is_some()
            && handle_agent_busy_key(
                *code,
                *kind,
                *modifiers,
                &mut input,
                &mut history,
                &mut stdout,
            )
        {
            continue;
        }

        if session_state.pending_clear_confirm {
            match (*code, *kind) {
                (KeyCode::Tab, KeyEventKind::Press)
                    if !modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    continue;
                }
                (KeyCode::Char('y' | 'Y'), KeyEventKind::Press) => {
                    session_state.pending_clear_confirm = false;
                    session_state.last_user_prompt = None;
                    stdout.write(StdoutMessage::newline());
                    () = spawn_agent_reinstall(&mut tokio_runtime, agent_config.clone());
                    () = write_repl_response(&mut stdout, "(conversation cleared)");
                    stdout.write(StdoutMessage::from(prompt_symbol()));
                    () = input.clear_line();
                    continue;
                }
                (KeyCode::Char('n' | 'N'), KeyEventKind::Press) => {
                    session_state.pending_clear_confirm = false;
                    stdout.write(StdoutMessage::newline());
                    () = write_repl_response(&mut stdout, "(clear cancelled)");
                    stdout.write(StdoutMessage::from(prompt_symbol()));
                    () = input.clear_line();
                    continue;
                }
                _ => {
                    session_state.pending_clear_confirm = false;
                    stdout.write(StdoutMessage::newline());
                    () = input.redraw_line(&mut stdout, agent_config.as_ref());
                }
            }
        }

        if let Some(confirm) = input.tab.list_confirm.clone() {
            match (*code, *kind) {
                (KeyCode::Tab, KeyEventKind::Press)
                    if !modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    continue;
                }
                (KeyCode::Char('y' | 'Y'), KeyEventKind::Press) => {
                    () = input.accept_tab_list(&mut stdout, confirm, agent_config.as_ref());
                    continue;
                }
                (KeyCode::Char('n' | 'N'), KeyEventKind::Press) => {
                    input.tab.list_confirm = None;
                    stdout.write(StdoutMessage::newline());
                    () = input.redraw_line(&mut stdout, agent_config.as_ref());
                    continue;
                }
                _ => {
                    input.tab.list_confirm = None;
                    stdout.write(StdoutMessage::newline());
                    () = input.redraw_line(&mut stdout, agent_config.as_ref());
                }
            }
        }

        match *code {
            KeyCode::Up
                if *kind == KeyEventKind::Press && history_keys_allowed(&session_state, &input) =>
            {
                if let Some(line) = history.recall_up(&input.content) {
                    () = input.apply_history_line(line, &mut stdout, agent_config.as_ref());
                }
            }
            KeyCode::Down
                if *kind == KeyEventKind::Press && history_keys_allowed(&session_state, &input) =>
            {
                if let Some(line) = history.recall_down() {
                    () = input.apply_history_line(line, &mut stdout, agent_config.as_ref());
                }
            }
            KeyCode::Tab
                if *kind == KeyEventKind::Press && !modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if history.is_recalling() {
                    () = history.cancel_recall();
                }
                () = input.handle_tab(&mut stdout, agent_config.as_ref());
            }
            KeyCode::Char(c)
                if (*kind == KeyEventKind::Press || *kind == KeyEventKind::Repeat)
                    && !modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if history.is_recalling() {
                    () = history.cancel_recall();
                }
                () = input.clear_tab_state();
                let mut buf = [0_u8; 4];
                let bytes = c.encode_utf8(&mut buf);

                if input.cursor == input.content.chars().count() {
                    () = input.clear_inline_hint(&mut stdout);
                    stdout.write(StdoutMessage::from(bytes));
                    input.cursor += 1;
                    () = input.content.push(c);
                    () = input.sync_hint(&mut stdout, agent_config.as_ref());
                } else if let Some((byte_idx, _c)) = input.content.char_indices().nth(input.cursor)
                {
                    let suffix = &input.content[byte_idx..];
                    let move_back_width: usize = suffix
                        .chars()
                        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                        .sum();
                    stdout.write(StdoutMessage::from(bytes));
                    stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
                    stdout.write(StdoutMessage::from(suffix));
                    for _ in 0..move_back_width {
                        stdout.write(StdoutMessage::move_cursor_left());
                    }
                    input.cursor += 1;
                    () = input.content.insert(byte_idx, c);
                    () = input.sync_hint(&mut stdout, agent_config.as_ref());
                }
            }
            KeyCode::Backspace => {
                if history.is_recalling() {
                    () = history.cancel_recall();
                }
                () = input.clear_tab_state();
                () = input.clear_inline_hint(&mut stdout);
                let cursor_pos = input.cursor;
                if cursor_pos != 0
                    && let Some((byte_idx, c)) = input.content.char_indices().nth(cursor_pos - 1)
                    && let Some(width) = UnicodeWidthChar::width(c)
                {
                    input.content.remove(byte_idx);
                    let suffix = &input.content[byte_idx..];
                    let move_back_width: usize = suffix
                        .chars()
                        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                        .sum();

                    for _ in 0..width {
                        stdout.write(StdoutMessage::move_cursor_left());
                    }
                    stdout.write(StdoutMessage::clear_line_from_cursor_to_end());
                    stdout.write(StdoutMessage::from(suffix));
                    for _ in 0..move_back_width {
                        stdout.write(StdoutMessage::move_cursor_left());
                    }
                    input.cursor = cursor_pos - 1;
                    () = input.sync_hint(&mut stdout, agent_config.as_ref());
                }
            }
            KeyCode::Left => {
                if history.is_recalling() {
                    () = history.cancel_recall();
                }
                input.clear_tab_state();
                input.clear_inline_hint(&mut stdout);
                let cursor_pos = input.cursor;
                if cursor_pos != 0
                    && let Some((_byte_idx, c)) = input.content.char_indices().nth(cursor_pos - 1)
                    && let Some(width) = UnicodeWidthChar::width(c)
                {
                    for _ in 0..width {
                        stdout.write(StdoutMessage::move_cursor_left());
                    }
                    input.cursor = cursor_pos - 1;
                }
            }
            KeyCode::Right => {
                if history.is_recalling() {
                    () = history.cancel_recall();
                }
                () = input.clear_tab_state();
                let cursor_pos = input.cursor;
                if cursor_pos != input.content.len()
                    && let Some(c) = input.content.chars().nth(cursor_pos)
                    && let Some(width) = UnicodeWidthChar::width(c)
                {
                    for _ in 0..width {
                        stdout.write(StdoutMessage::move_cursor_right());
                    }
                    input.cursor = cursor_pos + 1;
                    () = input.sync_hint(&mut stdout, agent_config.as_ref());
                }
            }
            KeyCode::Enter if !input.content.is_empty() => {
                () = input.clear_tab_state();
                () = input.clear_inline_hint(&mut stdout);
                () = history.push_submitted(&input.content);
                // Bang `!<cmd>` is a shell shortcut (yoyo): rewrite to `/run` before slash dispatch.
                let submitted = input.content.trim_start();
                let slash_line: Option<String> =
                    if let Some(body) = shell_run::parse_bang_command(submitted) {
                        Some(if body.is_empty() {
                            String::from("/run")
                        } else {
                            format!("/run {body}")
                        })
                    } else if submitted.starts_with('/') {
                        Some(String::from(submitted))
                    } else {
                        None
                    };
                if let Some(line) = slash_line.as_deref() {
                    let runtime = tokio_runtime.runtime();
                    let (cmd, _) = command_name_and_args(line);
                    let route = route_command(cmd);
                    let clear_stats = match route {
                        CommandRoute::Clear => coding_agent
                            .as_deref()
                            .map(|agent| block_on_session(runtime, agent_message_stats(agent))),
                        _ => None,
                    };
                    let dashboard = if route.is_info() {
                        let session_id = coding_agent
                            .as_deref()
                            .map(crate::agents::CodingAgent::session_id)
                            .unwrap_or_default();
                        let turn_rows = ecs_dashboard.turn_rows(session_id);
                        let meta = ecs_dashboard.meta_fields(session_id);
                        let context_stats = ecs_dashboard.context_stats_fields(session_id);
                        Some(build_snapshot(
                            &turn_rows,
                            meta.as_ref(),
                            &agent_config.model,
                            context_stats.as_ref(),
                            coding_agent.as_deref(),
                            runtime,
                            &ecs_dashboard.lifetime_usage.usage(),
                        ))
                    } else {
                        None
                    };
                    let session_processing = coding_agent
                        .as_ref()
                        .map(|agent| ecs_dashboard.is_processing(agent.session_id()))
                        .unwrap_or(false);
                    match dispatch_slash_command(
                        line,
                        agent_config.as_mut(),
                        session_state.as_mut(),
                        config.as_ref(),
                        clear_stats,
                        coding_agent.as_deref(),
                        runtime,
                        dashboard,
                        cli.bare,
                        session_processing,
                    ) {
                        DispatchResult::Exit => {
                            session_state.leaving = true;
                            () = persist_repl_history(
                                history.as_mut(),
                                &crate::config_paths::repl_history_path(),
                            );
                            () = commands.remove_resource::<CodingAgentTask>();
                            () = persist_session_on_exit(coding_agent.as_deref(), runtime);
                            () = write_quit_farewell_if_enabled(&mut stdout, cli.as_ref());
                            exit.write_default();
                            return;
                        }
                        DispatchResult::Handled {
                            output,
                            detail,
                            redraw_prompt,
                            reinstall,
                        } => {
                            stdout.write(StdoutMessage::newline());
                            if reinstall.is_some() {
                                session_state.last_user_prompt = None;
                            }
                            if let Some(config) = reinstall {
                                () = spawn_agent_reinstall(&mut tokio_runtime, config);
                            }
                            () = write_repl_handled_output(&mut stdout, &output, &detail);
                            if redraw_prompt {
                                stdout.write(StdoutMessage::from(prompt_symbol()));
                                () = input.clear_line();
                            } else if session_state.active_shell.is_some() {
                                // `/run` started: clear the submitted line; prompt returns on poll.
                                () = input.clear_line();
                            }
                        }
                        DispatchResult::AwaitClearConfirm { prompt } => {
                            stdout.write(StdoutMessage::newline());
                            () = write_repl_response(&mut stdout, &prompt);
                            session_state.pending_clear_confirm = true;
                            () = input.clear_line();
                        }
                        DispatchResult::ResendPrompt { prompt, hint } => {
                            stdout.write(StdoutMessage::newline());
                            () = write_repl_response(&mut stdout, &hint);
                            session_state.ctrl_c_armed = false;
                            () = prompt_channel.send_prompt(prompt);
                            stdout.write(StdoutMessage::newline());
                            () = input.clear_line();
                        }
                        DispatchResult::AgentOp(AgentOpInvocation { op, preamble }) => {
                            stdout.write(StdoutMessage::newline());
                            () = write_repl_response_lines(&mut stdout, &preamble);
                            () = spawn_agent_op(
                                &mut tokio_runtime,
                                op,
                                output_channel.as_ref(),
                                coding_agent.as_deref().cloned(),
                            );
                            () = input.clear_line();
                        }
                        DispatchResult::Unknown => {
                            stdout.write(StdoutMessage::newline());
                            let feedback = build_unknown_slash_feedback(line);
                            () = write_unknown_slash_feedback(
                                &mut stdout,
                                &feedback.typed,
                                feedback.suggestion,
                            );
                            stdout.write(StdoutMessage::from(prompt_symbol()));
                            () = input.clear_line();
                        }
                    }
                } else {
                    session_state.last_user_prompt = Some(input.content.clone());
                    session_state.ctrl_c_armed = false;
                    () = prompt_channel.send_prompt(input.content.clone());
                    stdout.write(StdoutMessage::newline());
                    () = input.clear_line();
                }
            }
            _ => (),
        }
    }
}

fn shutdown_repl(
    mut messages: MessageReader<AppExit>,
    mut history: ResMut<ReplInputHistory>,
    coding_agent: Option<Res<CodingAgent>>,
    tokio_runtime: Res<TokioTasksRuntime>,
    session_state: Option<Res<ReplSessionState>>,
) {
    for _message in messages.read() {
        // History always; session auto-save only if leave path did not already run it
        // (`leaving` set in leave_repl_app / Exit already saved).
        () = persist_repl_history(history.as_mut(), &crate::config_paths::repl_history_path());
        let already_saved = session_state.as_ref().is_some_and(|s| s.leaving);
        if !already_saved {
            persist_session_on_exit(coding_agent.as_deref(), tokio_runtime.runtime());
        }
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub(crate) fn repl_plugin(app: &mut App) {
    app.insert_resource(ReplInputHistory::load(
        &crate::config_paths::repl_history_path(),
        DEFAULT_MAX_ENTRIES,
    ))
    .init_resource::<ReplSessionState>()
    .init_resource::<ReplOutputChannel>()
    .add_systems(Startup, setup)
    .add_systems(
        PreUpdate,
        print_system_prompt.run_if(|cli: Res<Cli>| cli.print_system_prompt),
    )
    .add_systems(
        Update,
        (
            ctrl_c,
            poll_active_shell_run,
            echo_extern_prompt_submissions,
            read_stdin_stream,
        )
            .chain(),
    )
    // After `shutdown_tokio_on_exit`: stdin poll must drop `AppCancelToken` before
    // `disable_raw_mode` (concurrent raw-mode teardown vs crossterm poll can hang).
    .add_systems(
        PostUpdate,
        shutdown_repl.after(crate::tokio::shutdown_tokio_on_exit),
    );
}
