//! Bevy REPL plugin: stdin loop, slash dispatch, tab completion, async agent ops.
//!
//! Items are ordered for bottom-up reading: the file entry ([`repl_plugin`]) is last.
//! Each function sits directly above its callers; local callees are placed immediately
//! above the function that references them, in source appearance order. Reuse earlier
//! definitions when a callee is already defined above.

mod commands_help;
mod commands_lifecycle;
mod commands_session;
mod completion;
mod cost;
mod dispatch;
pub(crate) mod help_data;
mod model_cmd;
mod model_id;
mod native_pricing;
mod output;
mod path_display;
mod route;
mod session_ops;
mod session_state;
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
        stdin::StdinKeyMessage,
        stdout::StdoutMessage,
        tokio::AppCancelToken,
    },
    bevy::{
        app::{App, AppExit, PostUpdate, PreUpdate, Startup, Update},
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageReader, MessageWriter},
            schedule::{IntoScheduleConfigs, common_conditions::resource_removed},
            system::{Commands, Local},
        },
    },
    bevy_ratatui::crossterm::{
        self,
        event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    },
    bevy_tokio_tasks::TokioTasksRuntime,
    colored::Colorize,
    dispatch::{
        AgentOp, AgentOpInvocation, DispatchResult, command_name_and_args, dispatch_slash_command,
        unknown_command_message,
    },
    output::ReplOutputChannel,
    route::{CommandRoute, route_command},
    session_ops::{agent_message_stats_blocking, compact_agent, load_messages, save_messages},
    session_state::ReplSessionState,
    tab::{ReplTabLocals, TabListConfirm, accept_tab_candidate_list, handle_tab_completion},
    terminal::{
        redraw_input_line, sync_inline_hint, write_quit_farewell_if_enabled,
        write_repl_handled_output, write_repl_response, write_repl_response_lines,
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
    cli: Res<Cli>,
) {
    for StdinKeyMessage(KeyEvent {
        code, modifiers, ..
    }) in stdin_key_message.read()
    {
        if *code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
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
        write_repl_response(stdout, &line);
        drained = true;
    }
    drained
}

pub fn prompt_symbol() -> String {
    <&str as Colorize>::bold("\n> ").green().to_string()
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
            AgentOp::Compact => match coding_agent {
                Some(agent) => compact_agent(&agent).await,
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
}

impl ReplInputLocals {
    fn reset_line(&mut self) {
        self.cursor = 0;
        () = self.content.clear();
        self.hint_width = 0;
    }

    fn clear_line(&mut self) {
        self.cursor = 0;
        () = self.content.clear();
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
        accept_tab_candidate_list(
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
        handle_tab_completion(
            &mut self.content,
            &mut self.cursor,
            stdout,
            agent_config,
            &mut self.tab,
            &mut self.hint_width,
        );
    }

    fn sync_hint(&mut self, stdout: &mut MessageWriter<StdoutMessage>, agent_config: &AgentConfig) {
        sync_inline_hint(
            stdout,
            &self.content,
            self.cursor,
            agent_config,
            &mut self.hint_width,
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
    cli: Res<Cli>,
) {
    if drain_repl_output(&output_channel, &mut stdout) {
        stdout.write(StdoutMessage::from(prompt_symbol()));
        () = input.reset_line();
    }

    for StdinKeyMessage(KeyEvent {
        code,
        modifiers,
        kind,
        ..
    }) in stdin_key_messages.read()
    {
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
            KeyCode::Tab
                if *kind == KeyEventKind::Press && !modifiers.contains(KeyModifiers::CONTROL) =>
            {
                () = input.handle_tab(&mut stdout, agent_config.as_ref());
            }
            KeyCode::Char(c)
                if (*kind == KeyEventKind::Press || *kind == KeyEventKind::Repeat)
                    && !modifiers.contains(KeyModifiers::CONTROL) =>
            {
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
                if input.content.trim_start().starts_with('/') {
                    let line = input.content.trim_start();
                    let clear_stats = match route_command(command_name_and_args(line).0) {
                        CommandRoute::Clear => {
                            coding_agent.as_deref().map(agent_message_stats_blocking)
                        }
                        _ => None,
                    };
                    match dispatch_slash_command(
                        line,
                        agent_config.as_mut(),
                        session_state.as_ref(),
                        config.as_ref(),
                        clear_stats,
                    ) {
                        DispatchResult::Exit => {
                            write_quit_farewell_if_enabled(&mut stdout, cli.as_ref());
                            commands.remove_resource::<CodingAgentTask>();
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
                            () = write_repl_response(&mut stdout, unknown_command_message());
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

fn show_prompt_symbol(mut stdout: MessageWriter<StdoutMessage>) {
    stdout.write(StdoutMessage::from(prompt_symbol()));
}

fn shutdown_repl(mut messages: MessageReader<AppExit>) {
    for _message in messages.read() {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub(crate) fn repl_plugin(app: &mut App) {
    app.init_resource::<ReplSessionState>()
        .init_resource::<ReplOutputChannel>()
        .add_systems(Startup, setup)
        .add_systems(
            PreUpdate,
            print_system_prompt.run_if(|cli: Res<Cli>| cli.print_system_prompt),
        )
        .add_systems(Update, (ctrl_c, read_stdin_stream).chain())
        .add_systems(
            Update,
            show_prompt_symbol.run_if(resource_removed::<CodingAgentTask>),
        )
        .add_systems(PostUpdate, shutdown_repl);
}
