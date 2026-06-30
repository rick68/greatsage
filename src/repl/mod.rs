//! Bevy REPL plugin: stdin loop, slash dispatch, tab completion, async agent ops.
//!
//! Items are ordered for bottom-up reading: the file entry ([`repl_plugin`]) is last.
//! Each function sits directly above its callers; local callees are placed immediately
//! above the function that references them, in source appearance order. Reuse earlier
//! definitions when a callee is already defined above.

mod commands_help;
mod commands_info;
mod commands_lifecycle;
mod commands_session;
mod completion;
mod cost;
mod dispatch;
pub(crate) mod help_data;
mod history;
mod model_cmd;
mod model_id;
mod native_pricing;
mod output;
mod path_display;
mod route;
mod session_dashboard;
mod session_ops;
mod session_state;
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
        session::{
            SessionId, SessionLifetimeUsage, SessionManager, SessionMeta, TurnEntity, TurnSummary,
        },
        stdin::StdinKeyMessage,
        stdout::StdoutMessage,
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
    colored::Colorize,
    dispatch::{
        AgentOp, AgentOpInvocation, DispatchResult, build_unknown_slash_feedback,
        command_name_and_args, dispatch_slash_command,
    },
    history::{DEFAULT_MAX_ENTRIES, ReplInputHistory, persist_repl_history},
    output::ReplOutputChannel,
    route::{CommandRoute, route_command},
    session_dashboard::{SessionMetaFields, TurnUsageRow, build_snapshot},
    session_ops::{
        agent_message_stats, block_on_session, compact_agent_with_keep, load_messages,
        save_messages,
    },
    session_state::ReplSessionState,
    tab::{ReplTabLocals, TabListConfirm, accept_tab_candidate_list, handle_tab_completion},
    terminal::{
        erase_ahead_echo, redraw_input_line, replace_input_line_in_place, sync_inline_hint,
        write_quit_farewell_if_enabled, write_repl_handled_output, write_repl_response,
        write_repl_response_lines, write_unknown_slash_feedback,
    },
    unicode_width::UnicodeWidthChar,
};

fn setup(_app_cancel: Res<AppCancelToken>, _tokio_runtime: ResMut<TokioTasksRuntime>) {
    crossterm::terminal::enable_raw_mode().expect("Failed to enable raw mode");
}

fn print_system_prompt(
    config: Res<Config>,
    mut stdout: MessageWriter<StdoutMessage>,
    mut exit: MessageWriter<AppExit>,
) {
    let system_prompt = config.get_system_prompt();
    stdout.write(StdoutMessage::from(
        system_prompt.trim_end_matches([' ', '\t', '\n']),
    ));
    exit.write_default();
}

fn ctrl_c(
    mut commands: Commands,
    mut stdin_key_message: MessageReader<StdinKeyMessage>,
    mut exit: MessageWriter<AppExit>,
    mut stdout: MessageWriter<StdoutMessage>,
    mut history: ResMut<ReplInputHistory>,
    cli: Res<Cli>,
) {
    for StdinKeyMessage(KeyEvent {
        code, modifiers, ..
    }) in stdin_key_message.read()
    {
        if *code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            () = persist_repl_history(history.as_mut(), &crate::config_paths::repl_history_path());
            () = commands.remove_resource::<CodingAgentTask>();
            () = write_quit_farewell_if_enabled(&mut stdout, cli.as_ref());
            exit.write_default();
        }
    }
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

pub fn prompt_symbol() -> String {
    <&str as Colorize>::bold("\n> ").green().to_string()
}

pub(crate) fn prompt_symbol_inline() -> String {
    <&str as Colorize>::bold("> ").green().to_string()
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
            AgentOp::Load { path } => match coding_agent {
                Some(agent) => load_messages(&agent, &path).await,
                None => Err(String::from("No active agent.")),
            },
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
            } else if !input.content.is_empty() {
                () = input.sync_ahead_echo_full(stdout);
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
    if input.was_agent_busy && !agent_busy {
        () = restore_prompt_with_input(&mut stdout, &mut input, agent_config.as_ref());
    }
    input.was_agent_busy = agent_busy;

    if drain_repl_output(&output_channel, &mut stdout) {
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
                (KeyCode::Char(c), KeyEventKind::Press) if matches!(c, 'y' | 'Y') => {
                    session_state.pending_clear_confirm = false;
                    session_state.last_user_prompt = None;
                    stdout.write(StdoutMessage::newline());
                    () = spawn_agent_reinstall(&mut tokio_runtime, agent_config.clone());
                    () = write_repl_response(&mut stdout, "(conversation cleared)");
                    stdout.write(StdoutMessage::from(prompt_symbol()));
                    () = input.clear_line();
                    continue;
                }
                (KeyCode::Char(c), KeyEventKind::Press) if matches!(c, 'n' | 'N') => {
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
                (KeyCode::Char(c), KeyEventKind::Press) if matches!(c, 'y' | 'Y') => {
                    () = input.accept_tab_list(&mut stdout, confirm, agent_config.as_ref());
                    continue;
                }
                (KeyCode::Char(c), KeyEventKind::Press) if matches!(c, 'n' | 'N') => {
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
                if input.content.trim_start().starts_with('/') {
                    let line = input.content.trim_start();
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
                        Some(build_snapshot(
                            &turn_rows,
                            meta.as_ref(),
                            &agent_config.model,
                            coding_agent.as_deref(),
                            runtime,
                            &ecs_dashboard.lifetime_usage.0,
                        ))
                    } else {
                        None
                    };
                    match dispatch_slash_command(
                        line,
                        agent_config.as_mut(),
                        session_state.as_ref(),
                        config.as_ref(),
                        clear_stats,
                        coding_agent.as_deref(),
                        runtime,
                        dashboard,
                    ) {
                        DispatchResult::Exit => {
                            () = persist_repl_history(
                                history.as_mut(),
                                &crate::config_paths::repl_history_path(),
                            );
                            () = write_quit_farewell_if_enabled(&mut stdout, cli.as_ref());
                            () = commands.remove_resource::<CodingAgentTask>();
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
                                coding_agent.as_deref().map(|a| a.clone()),
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
                    () = prompt_channel.send_prompt(input.content.clone());
                    stdout.write(StdoutMessage::newline());
                    () = input.clear_line();
                }
            }
            _ => (),
        }
    }
}

fn shutdown_repl(mut messages: MessageReader<AppExit>, mut history: ResMut<ReplInputHistory>) {
    for _message in messages.read() {
        () = persist_repl_history(history.as_mut(), &crate::config_paths::repl_history_path());
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
    .add_systems(Update, (ctrl_c, read_stdin_stream).chain())
    .add_systems(PostUpdate, shutdown_repl);
}
