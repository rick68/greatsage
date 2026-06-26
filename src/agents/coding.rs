use {
    crate::{
        agents::{AgentConfig, AgentsCancelToken},
        cli::Cli,
        config::{Config, McpConfig},
        providers::Provider,
        repl::prompt_symbol,
        session::{
            AgentId, FocusedSession, SessionId, SessionManager, SessionRuntimeStatus,
            spawn_session_root, sync_session_meta, teardown_session,
        },
        setup,
        stdout::StdoutMessage,
        tokio::AppCancelToken,
        utils::{format_usage_line, truncate},
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
        state::{
            app::AppExtStates,
            condition::in_state,
            state::{NextState, States},
        },
    },
    bevy_ratatui::crossterm::terminal,
    bevy_tokio_tasks::TokioTasksRuntime,
    colored::Colorize,
    std::{
        env, fs,
        io::{self, IsTerminal},
        ops::{Deref, DerefMut},
        sync::Arc,
    },
    tokio::sync::Mutex,
    yoagent::{
        agent::Agent,
        provider::{AnthropicProvider, GoogleProvider, ModelConfig, OpenAiCompatProvider},
        tools::default_tools,
        types::{
            AgentEvent, AgentMessage, Content, Message as LlmMessage, StopReason, StreamDelta,
            Usage,
        },
    },
};

pub const SYSTEM_PROMPT: &str = r#"You are a coding assistant working in the user's terminal.
You have access to the filesystem and shell. Be direct and concise.
When the user asks you to do something, do it — don't just explain how.
Use tools proactively: read files to understand context, run commands to verify your work.
After making changes, run tests or verify the result when appropriate.
"#;

#[derive(Clone, Resource)]
pub struct CodingAgent {
    inner: Arc<Mutex<Agent>>,
    agent_id: AgentId,
    session_id: SessionId,
}

impl Deref for CodingAgent {
    type Target = Arc<Mutex<Agent>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for CodingAgent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl CodingAgent {
    #[allow(dead_code)]
    pub fn agent_id(&self) -> AgentId {
        self.agent_id
    }

    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    fn bind_session(&mut self, session_id: SessionId) {
        self.session_id = session_id;
    }

    pub async fn new_with_agent_config(agent_config: &AgentConfig) -> Self {
        let AgentConfig {
            model,
            provider,
            base_url,
            skills,
            system_prompt,
            api_key,
            mcp,
            ..
        } = agent_config;

        let (agent, model_config) = match provider {
            Provider::Anthropic => (
                Agent::new(AnthropicProvider),
                ModelConfig::anthropic(model, model),
            ),
            Provider::Cerebras => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::openai(model, model),
            ),
            Provider::Custom => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::local(base_url, model),
            ),
            Provider::DeepSeek => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::deepseek(model, model),
            ),
            Provider::Google => (
                Agent::new(GoogleProvider),
                ModelConfig::google(model, model),
            ),
            Provider::Groq => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::groq(model, model),
            ),
            Provider::MiniMax => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::minimax(model, model),
            ),
            Provider::Mistral => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::mistral(model, model),
            ),
            Provider::OpenAi => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::openai(model, model),
            ),
            Provider::OpenRouter => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::openai(model, model),
            ),
            Provider::Xai => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::xai(model, model),
            ),
            Provider::Zai => (
                Agent::new(OpenAiCompatProvider),
                ModelConfig::zai(model, model),
            ),
        };

        let mut agent = agent
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

        let inner = Arc::new(Mutex::new(agent));
        let agent_id = AgentId::of(&inner);

        Self {
            inner,
            agent_id,
            session_id: SessionId::default(),
        }
    }
}

/// Build a replacement agent with the same yoagent messages as `existing`.
pub async fn prepare_coding_agent_preserving_messages(
    existing: &CodingAgent,
    agent_config: &AgentConfig,
    success_message: String,
) -> Result<(CodingAgent, String), String> {
    let saved = existing
        .lock()
        .await
        .save_messages()
        .map_err(|e| format!("failed to save messages: {e}"))?;

    let coding_agent = CodingAgent::new_with_agent_config(agent_config).await;
    coding_agent
        .lock()
        .await
        .restore_messages(&saved)
        .map_err(|e| format!("failed to restore messages: {e}"))?;

    Ok((coding_agent, success_message))
}

/// Insert a new agent, allocate its session, and tear down any prior agent session.
pub(crate) fn install_coding_agent(
    world: &mut World,
    agent: CodingAgent,
    model: String,
    provider: String,
) {
    if let Some(old) = world.remove_resource::<CodingAgent>() {
        () = teardown_session(world, old.session_id());
    }

    let mut agent = agent;
    let (session_id, _) = spawn_session_root(world);
    () = agent.bind_session(session_id);
    () = sync_session_meta(world, session_id, model, provider);
    () = world.resource_mut::<FocusedSession>().set(session_id);
    () = world.insert_resource(agent);
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
    let model = agent_config.model.clone();
    let provider = agent_config.provider.to_string();

    tokio_runtime.spawn_background_task(move |mut ctx| async move {
        let agent_config = AgentConfig::from(&config);
        let coding_agent = CodingAgent::new_with_agent_config(&agent_config).await;
        ctx.run_on_main_thread(move |ctx| {
            () = install_coding_agent(ctx.world, coding_agent, model, provider);
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

    if !cli.no_hints {
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
        if setup::needs_setup() {
            stdout.write(StdoutMessage::from(
                "  hint: no API key configured — run `greatsage setup`\n".dimmed(),
            ));
        }
    }

    if cli.prompt.is_none() && !cli.print_system_prompt && io::stdin().is_terminal() {
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
    in_thinking: bool,
    /// Tracks whether we have already displayed thinking content during this response
    /// (via StreamDelta::Thinking). Used to avoid duplicating Content::Thinking in MessageEnd.
    thinking_shown: bool,
}

/// Bevy Message (buffered) — introduced as a distinct concept in Bevy 0.17.
///
/// Since Bevy 0.17:
/// - `Event` trait is reserved for observable/reactive events (Observer, trigger, On<Event>).
/// - `Message` trait is for classic buffered, high-throughput fire-and-forget messages.
///
/// `CodingAgentEvent` uses the Message system because it carries a continuous stream
/// of yoagent events (text/thinking/tool deltas + final usage) from the LLM agent
/// into Bevy systems. This is the correct choice for this use case.
#[derive(Debug, Message)]
pub struct CodingAgentEvent {
    pub session_id: SessionId,
    pub event: AgentEvent,
}

impl std::ops::Deref for CodingAgentEvent {
    type Target = AgentEvent;

    fn deref(&self) -> &Self::Target {
        &self.event
    }
}

fn spawn_agent_task(
    coding_agent: Option<ResMut<CodingAgent>>,
    channel: Res<CodingAgentPromptChannel>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
    session_manager: Res<SessionManager>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<CodingAgentState>>,
) {
    if let Some(coding_agent) = coding_agent
        && let Ok(prompt) = channel.receiver.try_recv()
    {
        let coding_agent = coding_agent.clone();
        let session_id = coding_agent.session_id();

        if let Some(root) = session_manager.root_entity(session_id) {
            commands
                .entity(root)
                .insert(SessionRuntimeStatus::processing());
        }

        tokio_runtime.spawn_background_task(move |mut ctx| async move {
            let mut rx = coding_agent.lock().await.prompt(prompt).await;

            while let Some(event) = rx.recv().await {
                () = ctx
                    .run_on_main_thread(move |ctx| {
                        ctx.world
                            .write_message::<CodingAgentEvent>(CodingAgentEvent {
                                session_id,
                                event,
                            });
                    })
                    .await;
            }

            ctx.run_on_main_thread(move |ctx| {
                let world: &mut World = ctx.world;
                world.remove_resource::<CodingAgentTask>();

                if let Some(root) = world.resource::<SessionManager>().root_entity(session_id)
                    && let Ok(mut entity) = world.get_entity_mut(root)
                    && let Some(mut status) = entity.get_mut::<SessionRuntimeStatus>()
                {
                    status.set_idle();
                }

                world
                    .get_resource_mut::<NextState<CodingAgentState>>()
                    .unwrap()
                    .set(CodingAgentState::Idle);
            })
            .await;
        });

        () = commands.init_resource::<CodingAgentTask>();
        () = next_state.set(CodingAgentState::Processing);
    }
}

fn usage_info(usage: &Usage) -> String {
    format_usage_line(usage)
        .map(|line| format!("\n\n  {line}\n").dimmed().to_string())
        .unwrap_or_default()
}

/// Dimmed full-width header for the start of a thinking block.
/// Newline before calling this is the caller's responsibility.
fn thinking_header() -> StdoutMessage {
    let (width, _height) = terminal::size().unwrap_or((80, 24));
    StdoutMessage::from(format!("── Thinking {}\n", "─".repeat(width as usize - 12)).dimmed())
}

/// Dimmed full-width divider for the end of a thinking block.
/// Caller should ensure the previous content ends with a newline if needed.
fn thinking_divider() -> StdoutMessage {
    let (width, _height) = terminal::size().unwrap_or((80, 24));
    StdoutMessage::from(format!("{}\n", "─".repeat(width as usize)).dimmed())
}

/// Extracts the thinking text from a streaming `StreamDelta`, if it is a Thinking delta.
fn extract_thinking_from_delta(delta: &StreamDelta) -> Option<String> {
    match delta {
        StreamDelta::Thinking { delta } => Some(delta.clone()),
        _ => None,
    }
}

/// Extracts the thinking text from the final assistant message content, if present.
/// Returns the text of the first `Content::Thinking` variant found.
fn extract_thinking_from_final_content(content: &[Content]) -> Option<String> {
    content.iter().find_map(|c| {
        if let Content::Thinking { thinking, .. } = c {
            Some(thinking.clone())
        } else {
            None
        }
    })
}

fn assistant_error_line(message: &LlmMessage) -> Option<String> {
    let LlmMessage::Assistant {
        stop_reason,
        error_message,
        ..
    } = message
    else {
        return None;
    };
    match stop_reason {
        StopReason::Error => {
            let detail = error_message.as_deref().unwrap_or("unknown provider error");
            Some(format!("\n  error: {detail}\n").red().to_string())
        }
        StopReason::Aborted => Some("\n  error: request aborted\n".red().to_string()),
        _ => None,
    }
}

fn empty_assistant_hint(message: &LlmMessage) -> Option<String> {
    let LlmMessage::Assistant {
        content,
        usage,
        stop_reason,
        ..
    } = message
    else {
        return None;
    };
    if *stop_reason == StopReason::Error || *stop_reason == StopReason::Aborted {
        return None;
    }
    let has_text = content
        .iter()
        .any(|block| matches!(block, Content::Text { text } if !text.trim().is_empty()));
    if has_text || usage.total_tokens > 0 {
        return None;
    }
    Some(
        "\n  error: model returned no content (check model id and base_url)\n"
            .red()
            .to_string(),
    )
}

/// Bevy buffered Message handling (confirmed for this change).
///
/// Since Bevy 0.17, the engine distinguishes:
/// - `Message` (buffered, via MessageReader/MessageWriter) — what we use here.
/// - `Event` (observable/reactive, via Observer) — intentionally not used for agent streaming.
///
/// This function is the single place where yoagent events (including future
/// StreamDelta::Thinking and Content::Thinking handling) are turned into
/// visible StdoutMessage output.
fn handle_coding_agent_events(
    mut messages: MessageReader<CodingAgentEvent>,
    mut coding_agent_task: ResMut<CodingAgentTask>,
    mut stdout: MessageWriter<StdoutMessage>,
    cli: Res<Cli>,
) {
    for msg in messages.read() {
        let event = &msg.event;
        let CodingAgentTask {
            last_usage,
            in_text,
            in_thinking,
            thinking_shown,
            ..
        } = coding_agent_task.as_mut();

        match event {
            AgentEvent::InputRejected { reason } => {
                stdout.write(StdoutMessage::from(
                    format!("\n  error: input rejected: {reason}\n")
                        .red()
                        .to_string(),
                ));
            }
            AgentEvent::TurnEnd { message, .. } => {
                if let AgentMessage::Llm(llm_message) = &message {
                    if let Some(line) = assistant_error_line(llm_message)
                        .or_else(|| empty_assistant_hint(llm_message))
                    {
                        stdout.write(StdoutMessage::from(line));
                    }
                }
            }
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
                if !cli.no_hints {
                    stdout.write(StdoutMessage::from(<&str as Colorize>::yellow(
                        format!("\n  ▶ {summary}").as_str(),
                    )));
                }
            }
            AgentEvent::ToolExecutionEnd { is_error, .. } if !cli.no_hints => {
                // Append the symbol without extra newline here; the caller (or next output)
                // is responsible for spacing. This matches the original tool status style.
                if *is_error {
                    stdout.write(StdoutMessage::from(<&str as Colorize>::red(" ✗")));
                } else {
                    stdout.write(StdoutMessage::from(<&str as Colorize>::green(" ✓")));
                }
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Text { delta },
                ..
            } => {
                // Exit thinking block when the first normal text delta arrives.
                // We add a newline + divider here because the last thinking delta
                // from the provider often does not end with '\n'.
                if *in_thinking {
                    if !cli.no_hints {
                        stdout.write(StdoutMessage::newline());
                        stdout.write(thinking_divider());
                    }
                    *in_thinking = false;
                }

                if !*in_text {
                    if !cli.no_hints {
                        stdout.write(StdoutMessage::newline());
                    }
                    *in_text = true;
                }
                stdout.write(StdoutMessage::from(delta));
            }
            AgentEvent::MessageUpdate { delta, .. } if !cli.no_hints => {
                if let Some(thinking_text) = extract_thinking_from_delta(delta) {
                    if thinking_text.trim().is_empty() {
                        // Ignore empty thinking deltas to avoid blank thinking blocks.
                        continue;
                    }

                    if !*in_thinking {
                        // First thinking delta of this block → print header.
                        // We rely on previous output (tool result or previous turn) to have ended its line.
                        stdout.write(thinking_header());
                        *in_thinking = true;
                        *thinking_shown = true;
                    }

                    // When we receive thinking, we are no longer in normal text output mode.
                    if *in_text {
                        *in_text = false;
                    }

                    // Print thinking content dimmed (no extra newlines — deltas are incremental).
                    stdout.write(StdoutMessage::from(thinking_text.dimmed()));
                }
            }
            AgentEvent::MessageEnd {
                message: AgentMessage::Llm(yoagent::types::Message::Assistant { content, .. }),
                ..
            } => {
                // Non-streaming fallback: if the entire response came back as one message
                // containing Content::Thinking, we render it here.
                // We only do this if we didn't already render via streaming deltas.
                if !cli.no_hints
                    && !*thinking_shown
                    && let Some(thinking_text) = extract_thinking_from_final_content(content)
                {
                    if thinking_text.trim().is_empty() {
                        // Skip empty thinking content to avoid blank thinking blocks.
                        continue;
                    }

                    // Non-streaming path: add a leading newline before the header for spacing.
                    stdout.write(thinking_header());
                    stdout.write(StdoutMessage::from(thinking_text.dimmed()));
                    stdout.write(thinking_divider());
                }

                if let Some(output) = cli.output.as_ref() {
                    for cnt in content.iter() {
                        if let Content::Text { text } = cnt {
                            let _ = fs::write(output, text.clone() + "\n");
                        }
                    }
                }
            }
            AgentEvent::AgentEnd { messages } => {
                // Reset thinking-related state at the end of a turn
                if *in_thinking {
                    *in_thinking = false;
                }
                *thinking_shown = false;

                let mut usage_printed = false;
                for msg in messages.iter().rev() {
                    let AgentMessage::Llm(llm_message) = msg else {
                        continue;
                    };
                    if !usage_printed {
                        if let Some(line) = assistant_error_line(llm_message)
                            .or_else(|| empty_assistant_hint(llm_message))
                        {
                            stdout.write(StdoutMessage::from(line));
                        }
                        if let LlmMessage::Assistant { usage, .. } = llm_message {
                            if cli.no_hints {
                                stdout.write(StdoutMessage::newline());
                            } else {
                                stdout.write(StdoutMessage::from(usage_info(usage)));
                            }
                            *last_usage = usage.clone();
                            usage_printed = true;
                        }
                    }
                }
            }
            _ => {} // Note:
                    // - All streaming output uses Bevy Message (not Event).
                    // - Thinking output respects `no_hints` and uses `Colorize` + `StdoutMessage`.
                    // - `thinking_shown` prevents duplicate thinking between deltas and final Content::Thinking.
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
            () = cancel.cancel();
        }
    }
}

pub fn coding_agent_plugin(app: &mut App) {
    app.init_resource::<CodingAgentPromptChannel>()
        .add_systems(Startup, setup)
        .init_state::<CodingAgentState>()
        // Bevy 0.17+ buffered Message registration (not add_event / Event).
        // We use the Message system (not the new observable Event + Observer system)
        // because CodingAgentEvent carries high-volume streaming data from yoagent.
        .add_message::<CodingAgentEvent>()
        .add_systems(
            Update,
            (
                spawn_agent_task.run_if(
                    in_state(CodingAgentState::Idle)
                        .and_then(not(resource_exists::<CodingAgentTask>)),
                ),
                handle_coding_agent_events.run_if(
                    in_state(CodingAgentState::Processing)
                        .and_then(resource_exists::<CodingAgentTask>),
                ),
            ),
        )
        .add_systems(PostUpdate, shutdown_coding_agent);
}
