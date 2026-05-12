use {
    crate::{
        agents::{AgentConfig, CodingAgent, CodingAgentPromptChannel, CodingAgentTask},
        cli::Cli,
        config::Config,
        stdout::StdoutMessage,
        tokio::AppCancelToken,
    },
    bevy::{
        app::{App, AppExit, PreUpdate, Startup, Update},
        ecs::{
            change_detection::{Res, ResMut},
            message::{Message, MessageReader, MessageWriter},
            resource::Resource,
            schedule::{IntoScheduleConfigs, common_conditions::resource_removed},
            system::{Commands, Local},
        },
        prelude::Deref,
    },
    bevy_ratatui::crossterm::{
        self,
        event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    },
    bevy_tokio_tasks::TokioTasksRuntime,
    colored::Colorize,
    crossbeam_channel::{Receiver, unbounded},
    std::time::Duration,
    unicode_width::UnicodeWidthChar,
};

const STDIN_POLL_TIMEOUT_MS: u64 = 100;

#[derive(Message)]
struct StdinKeyMessage(KeyEvent);

#[derive(Deref, Resource)]
struct StreamReceiver(Receiver<KeyEvent>);

fn setup(
    mut commands: Commands,
    app_cancel: Res<AppCancelToken>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
) {
    let (tx, rx) = unbounded::<KeyEvent>();
    commands.insert_resource(StreamReceiver(rx));

    crossterm::terminal::enable_raw_mode().expect("Failed to enable raw mode");

    let app_cancel = app_cancel.clone();

    tokio_runtime.spawn_background_task(|_ctx| async move {
        let app_cancel = app_cancel.clone();

        tokio::task::spawn_blocking(move || {
            let timeout = Duration::from_millis(STDIN_POLL_TIMEOUT_MS);
            while !app_cancel.is_cancelled() {
                if event::poll(timeout).expect("Failed to poll stdin") {
                    let evt = event::read().expect("Failed to read stdin event");
                    if let event::Event::Key(key) = evt {
                        tx.send(key).expect("Failed to transmit key event");
                    }
                }
            }
        });
    });
}

fn forward_stdin_input_to_messsage(
    stdin_keys: ResMut<StreamReceiver>,
    mut messages: MessageWriter<StdinKeyMessage>,
) {
    while let Ok(key) = stdin_keys.try_recv() {
        messages.write(StdinKeyMessage(key));
    }
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

pub(crate) fn repl_plugin(app: &mut App) {
    app.add_message::<StdinKeyMessage>()
        .add_systems(Startup, setup)
        .add_systems(
            PreUpdate,
            (
                forward_stdin_input_to_messsage,
                print_system_prompt.run_if(|cli: Res<Cli>| cli.print_system_prompt),
            ),
        )
        .add_systems(Update, (ctrl_c, read_stdin_stream).chain())
        .add_systems(
            Update,
            show_prompt_symbol.run_if(resource_removed::<CodingAgentTask>),
        );
}
