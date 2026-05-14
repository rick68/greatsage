use {
    crate::{
        agents::{AgentConfig, AgentsCancelToken},
        cli::Cli,
        config::{Config, McpConfig},
        repl::prompt_symbol,
        stdout::StdoutMessage,
        tokio::AppCancelToken,
        utils::truncate,
    },
    bevy::{
        app::{App, AppExit, PostUpdate, Startup, Update},
        ecs::{
            change_detection::{Res, ResMut},
            message::{Message, MessageReader, MessageWriter},
            resource::Resource,
            schedule::{
                IntoScheduleConfigs, SystemCondition,
                common_conditions::{not, resource_exists},
            },
            system::Commands,
            world::World,
        },
        prelude::{Deref, DerefMut},
        state::{
            app::AppExtStates,
            condition::in_state,
            state::{NextState, States},
        },
    },
    bevy_tokio_tasks::TokioTasksRuntime,
    colored::Colorize,
    std::{env, fs, sync::Arc},
    tokio::sync::Mutex,
    yoagent::{
        agent::Agent,
        provider::{ModelConfig, openai_compat::OpenAiCompatProvider},
        tools::default_tools,
        types::{AgentEvent, AgentMessage, Content, StreamDelta, Usage},
    },
};

pub const SYSTEM_PROMPT: &str = r#"You are a coding assistant working in the user's terminal.
You have access to the filesystem and shell. Be direct and concise.
When the user asks you to do something, do it — don't just explain how.
Use tools proactively: read files to understand context, run commands to verify your work.
After making changes, run tests or verify the result when appropriate.
"#;

#[derive(Clone, Deref, DerefMut, Resource)]
pub struct CodingAgent(Arc<Mutex<Agent>>);

impl CodingAgent {
    pub async fn new_with_agent_config(agent_config: &AgentConfig) -> Self {
        let AgentConfig {
            base_url,
            model,
            skills,
            system_prompt,
            api_key,
            mcp,
            ..
        } = agent_config;

        let model_config = ModelConfig::local(base_url, model);
        let mut agent = Agent::new(OpenAiCompatProvider)
            .with_model_config(model_config)
            .with_system_prompt(system_prompt)
            .with_model(model)
            .with_api_key(api_key)
            .with_tools(default_tools());

        if !skills.is_empty() {
            agent = agent.with_skills(skills.clone());
        }

        if !mcp.is_empty() {
            for transport in mcp {
                agent = match transport {
                    McpConfig::SseTransports(url) => agent
                        .with_mcp_server_http(url.as_str())
                        .await
                        .expect("Failed to connect to MCP server"),
                    McpConfig::StdioTransports(cmd) => {
                        if let Ok(args) = shell_words::split(cmd.as_str())
                            && !args.is_empty()
                        {
                            let (command, args) = args.split_at(1);
                            agent
                                .with_mcp_server_stdio(
                                    &command[0],
                                    args.iter()
                                        .map(String::as_str)
                                        .collect::<Vec<&str>>()
                                        .as_slice(),
                                    None,
                                )
                                .await
                                .expect("Failed to connect to MCP server")
                        } else {
                            agent
                        }
                    }
                };
            }
        }

        CodingAgent(Arc::new(Mutex::new(agent)))
    }
}

#[derive(Clone, Resource)]
pub struct CodingAgentPromptChannel {
    pub sender: crossbeam_channel::Sender<String>,
    pub receiver: crossbeam_channel::Receiver<String>,
}

impl Default for CodingAgentPromptChannel {
    fn default() -> Self {
        let (sender, receiver): (
            crossbeam_channel::Sender<String>,
            crossbeam_channel::Receiver<String>,
        ) = crossbeam_channel::unbounded::<String>();

        Self { sender, receiver }
    }
}

impl CodingAgentPromptChannel {
    pub fn send_prompt(&self, text: impl Into<String>) {
        let _ = self.sender.send(text.into());
    }
}

fn banner() -> String {
    format!(
        "\n{} {}\n",
        <&str as Colorize>::bold("greatsage").cyan(),
        "— a coding agent growing up in public".dimmed()
    )
}

fn setup(
    config: Res<Config>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
    mut commands: Commands,
    app_cancel: Res<AppCancelToken>,
    agents_cancel: Res<AgentsCancelToken>,
    mut stdout: MessageWriter<StdoutMessage>,
    cli: Res<Cli>,
) {
    let config = config.clone();
    let agent_config = AgentConfig::from(&config);

    tokio_runtime.spawn_background_task(move |mut ctx| async move {
        let agent_config = AgentConfig::from(&config);
        let coding_agent = CodingAgent::new_with_agent_config(&agent_config).await;
        ctx.run_on_main_thread(move |ctx| {
            ctx.world.insert_resource::<CodingAgent>(coding_agent);
        })
        .await;
    });

    commands.init_resource::<CodingAgentPromptChannel>();

    let app_cancel = app_cancel.clone();
    let agents_cancel = agents_cancel.clone();

    tokio_runtime.spawn_background_task(|_ctx| async move {
        tokio::select! {
            _ = app_cancel.cancelled() => (),
            _ = agents_cancel.cancelled() => (),
            else => unreachable!(),
        }
    });

    stdout.write(StdoutMessage::from(banner()));
    stdout.write(StdoutMessage::from(
        format!("  model: {}\n", agent_config.model).dimmed(),
    ));
    if !agent_config.skills.is_empty() {
        stdout.write(StdoutMessage::from(
            format!("  skills: {} loaded\n", agent_config.skills.len()).dimmed(),
        ));
    }
    if !agent_config.mcp.is_empty() {
        stdout.write(StdoutMessage::from(
            format!("  mcp: {} server(s) connected\n", agent_config.mcp.len()).dimmed(),
        ));
    }
    if let Ok(cwd) = env::current_dir() {
        stdout.write(StdoutMessage::from(
            format!("  cwd: {}\n", cwd.display()).dimmed(),
        ));
    }
    if cli.prompt.is_none() && !cli.print_system_prompt {
        stdout.write(StdoutMessage::from(prompt_symbol()));
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq, States)]
enum CodingAgentState {
    #[default]
    Idle,
    Processing,
}

#[derive(Default, Resource)]
pub struct CodingAgentTask {
    last_usage: Usage,
    in_text: bool,
}

#[derive(Debug, Deref, DerefMut, Message)]
pub struct CodingAgentEvent(AgentEvent);

fn spawn_agent_task(
    coding_agent: Option<ResMut<CodingAgent>>,
    channel: Res<CodingAgentPromptChannel>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<CodingAgentState>>,
) {
    if let Some(coding_agent) = coding_agent
        && let Ok(prompt) = channel.receiver.try_recv()
    {
        let coding_agent = coding_agent.clone();

        tokio_runtime.spawn_background_task(move |mut ctx| async move {
            let mut rx = coding_agent.lock().await.prompt(prompt.clone()).await;

            while let Some(event) = rx.recv().await {
                () = ctx
                    .run_on_main_thread(|ctx| {
                        ctx.world
                            .write_message::<CodingAgentEvent>(CodingAgentEvent(event));
                    })
                    .await;
            }

            ctx.run_on_main_thread(|ctx| {
                let world: &mut World = ctx.world;
                world.remove_resource::<CodingAgentTask>();

                world
                    .get_resource_mut::<NextState<CodingAgentState>>()
                    .unwrap()
                    .set(CodingAgentState::Idle);
            })
            .await;
        });

        commands.init_resource::<CodingAgentTask>();
        next_state.set(CodingAgentState::Processing);
    }
}

fn usage_info(Usage { input, output, .. }: &Usage) -> String {
    if *input > 0 || *output > 0 {
        format!("\n\n  tokens: {input} in / {output} out\n")
            .dimmed()
            .to_string()
    } else {
        String::new()
    }
}

fn handle_coding_agent_events(
    mut messages: MessageReader<CodingAgentEvent>,
    mut coding_agent_task: ResMut<CodingAgentTask>,
    mut stdout: MessageWriter<StdoutMessage>,
    cli: Res<Cli>,
) {
    for CodingAgentEvent(event) in messages.read() {
        let CodingAgentTask {
            last_usage,
            in_text,
            ..
        } = coding_agent_task.as_mut();

        match event {
            AgentEvent::ToolExecutionStart {
                tool_name, args, ..
            } => {
                if *in_text {
                    *in_text = false;
                }
                let summary = match tool_name.as_str() {
                    "bash" => {
                        let cmd = args
                            .get("command")
                            .and_then(|v| v.as_str())
                            .unwrap_or("...");
                        format!("$ {}", truncate(cmd, 80))
                    }
                    "read_file" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("read {path}")
                    }
                    "write_file" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("write {path}")
                    }
                    "edit_file" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("edit {path}")
                    }
                    "list_files" => {
                        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                        format!("ls {path}")
                    }
                    "search" => {
                        let pat = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("search '{}'", truncate(pat, 60))
                    }
                    _ => tool_name.clone(),
                };
                stdout.write(StdoutMessage::from(<&str as Colorize>::yellow(
                    format!("  ▶ {summary}").as_str(),
                )));
            }
            AgentEvent::ToolExecutionEnd { is_error, .. } => {
                if *is_error {
                    stdout.write(StdoutMessage::from(<&str as Colorize>::red(" ✗\r\n")));
                } else {
                    stdout.write(StdoutMessage::from(<&str as Colorize>::green(" ✓\r\n")));
                }
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Text { delta },
                ..
            } => {
                if !*in_text {
                    stdout.write(StdoutMessage::newline());
                    *in_text = true;
                }
                stdout.write(StdoutMessage::from(delta));
            }
            AgentEvent::MessageEnd {
                message: AgentMessage::Llm(yoagent::types::Message::Assistant { content, .. }),
                ..
            } => {
                if let Some(output) = cli.output.as_ref() {
                    for cnt in content.iter() {
                        if let Content::Text { text } = cnt {
                            let _ = fs::write(output, text);
                        }
                    }
                }
            }
            AgentEvent::AgentEnd { messages } => {
                for msg in messages.iter().rev() {
                    if let AgentMessage::Llm(yoagent::types::Message::Assistant { usage, .. }) = msg
                    {
                        stdout.write(StdoutMessage::from(usage_info(usage)));
                        *last_usage = usage.clone();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
}

fn shutdown_coding_agent(
    mut messages: MessageReader<AppExit>,
    mut cancel: Option<Res<AgentsCancelToken>>,
) {
    for _message in messages.read() {
        if let Some(cancel) = cancel.take()
            && !cancel.is_cancelled()
        {
            cancel.cancel();
        }
    }
}

pub fn coding_agent_plugin(app: &mut App) {
    app.init_resource::<CodingAgentPromptChannel>()
        .add_systems(Startup, setup)
        .init_state::<CodingAgentState>()
        .add_message::<CodingAgentEvent>()
        .add_systems(
            Update,
            (
                spawn_agent_task.run_if(
                    in_state(CodingAgentState::Idle).and(not(resource_exists::<CodingAgentTask>)),
                ),
                handle_coding_agent_events.run_if(
                    in_state(CodingAgentState::Processing).and(resource_exists::<CodingAgentTask>),
                ),
            ),
        )
        .add_systems(PostUpdate, shutdown_coding_agent);
}
