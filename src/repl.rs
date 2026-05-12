use {
    crate::{
        agents::{AgentConfig, CodingAgent, CodingAgentPromptChannel, CodingAgentTask},
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
            message::{ MessageReader, MessageWriter},
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
    std::{
        io::{self, Write},
    },
    unicode_width::UnicodeWidthChar,
};

fn setup(app_cancel: Res<AppCancelToken>, tokio_runtime: ResMut<TokioTasksRuntime>, cli: Res<Cli>) {
    let print_system_prompt = cli.print_system_prompt;
    let app_cancel = app_cancel.clone();

    tokio_runtime.spawn_background_task(move |_ctx| async move {
        tokio::select! {
            _ = app_cancel.cancelled() => {
                if !print_system_prompt {
                    let mut lock = io::stdout();
                    let _ = lock.write("\r\n  bye 👋\r\n".dimmed().as_bytes());
                    let _ = lock.flush();
                }
            },
            else => unreachable!(),
        }
    });

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
) {
    for StdinKeyMessage(KeyEvent {
        code, modifiers, ..
    }) in stdin_key_message.read()
    {
        if *code == KeyCode::Char('c') && modifiers.contains(KeyModifiers::CONTROL) {
            commands.remove_resource::<CodingAgentTask>();
            exit.write_default();
            stdout.write(StdoutMessage::newline());
        }
    }
}

pub fn prompt_symbol() -> String {
    <&str as Colorize>::bold("\n> ").green().to_string()
}

fn show_prompt_symbol(mut stdout: MessageWriter<StdoutMessage>) {
    stdout.write(StdoutMessage::from(prompt_symbol()));
}

#[allow(clippy::too_many_arguments)]
fn read_stdin_stream(
    mut stdin_key_messages: MessageReader<StdinKeyMessage>,
    mut cursor: Local<usize>,
    mut content: Local<String>,
    mut stdout: MessageWriter<StdoutMessage>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
    prompt_channel: Res<CodingAgentPromptChannel>,
    mut agent_config: ResMut<AgentConfig>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
) {
    for StdinKeyMessage(KeyEvent {
        code,
        modifiers,
        kind,
        ..
    }) in stdin_key_messages.read()
    {
        match *code {
            KeyCode::Char(c)
                if (*kind == KeyEventKind::Press || *kind == KeyEventKind::Repeat)
                    && !modifiers.contains(KeyModifiers::CONTROL) =>
            {
                let mut buf = [0_u8; 4];
                let bytes = c.encode_utf8(&mut buf);

                if *cursor == content.chars().count() {
                    stdout.write(StdoutMessage::from(bytes));
                    *cursor += 1;
                    content.push(c);
                } else if let Some((byte_idx, _c)) = content.char_indices().nth(*cursor) {
                    let suffix = &content[byte_idx..];
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
                    *cursor += 1;
                    content.insert(byte_idx, c);
                }
            }
            KeyCode::Backspace => {
                let pos = &mut *cursor;
                if *pos != 0
                    && let Some((byte_idx, c)) = content.char_indices().nth(*pos - 1)
                    && let Some(width) = UnicodeWidthChar::width(c)
                {
                    content.remove(byte_idx);
                    let suffix = &content[byte_idx..];
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
                    *pos -= 1;
                }
            }
            KeyCode::Left => {
                let pos = &mut *cursor;
                if *pos != 0
                    && let Some((_byte_idx, c)) = content.char_indices().nth(*pos - 1)
                    && let Some(width) = UnicodeWidthChar::width(c)
                {
                    for _ in 0..width {
                        stdout.write(StdoutMessage::move_cursor_left());
                    }
                    *pos -= 1;
                }
            }
            KeyCode::Right => {
                let pos = &mut *cursor;
                if *pos != content.len()
                    && let Some(c) = content.chars().nth(*pos)
                    && let Some(width) = UnicodeWidthChar::width(c)
                {
                    for _ in 0..width {
                        stdout.write(StdoutMessage::move_cursor_right());
                    }
                    *pos += 1;
                }
            }
            KeyCode::Enter if !content.is_empty() => {
                if content.trim_start().starts_with("/") {
                    match content.trim_start() {
                        "/exit" | "/quit" => {
                            commands.remove_resource::<CodingAgentTask>();
                            exit.write_default();
                            stdout.write(StdoutMessage::newline());
                            return;
                        }
                        "/clear" => {
                            let agent_config = agent_config.clone();
                            tokio_runtime.spawn_background_task(|mut ctx| async move {
                                let coding_agent =
                                    CodingAgent::new_with_agent_config(&agent_config).await;
                                ctx.run_on_main_thread(|ctx| {
                                    ctx.world.insert_resource(coding_agent);
                                })
                                .await;
                            });
                        }
                        s if s.starts_with("/model ") => {
                            let agent_config = agent_config.as_mut();
                            let new_model = s.trim_start_matches("/model ").trim();
                            if new_model.is_empty() {
                                continue;
                            }
                            agent_config.model = String::from(new_model);

                            let agent_config = agent_config.clone();

                            tokio_runtime.spawn_background_task(|mut ctx| async move {
                                let coding_agent =
                                    CodingAgent::new_with_agent_config(&agent_config).await;
                                ctx.run_on_main_thread(|ctx| {
                                    ctx.world.insert_resource(coding_agent);
                                })
                                .await;
                            });
                        }
                        _ => continue,
                    }
                    stdout.write(StdoutMessage::newline());
                    stdout.write(StdoutMessage::from(prompt_symbol()));
                } else {
                    prompt_channel.send_prompt(content.clone());
                    stdout.write(StdoutMessage::newline());
                }
                *cursor = 0;
                content.clear();
            }
            _ => (),
        }
    }
}

fn shutdown_repl(mut messages: MessageReader<AppExit>) {
    for _message in messages.read() {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub(crate) fn repl_plugin(app: &mut App) {
    app.add_systems(Startup, setup)
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
