use {
    crate::{
        agents::tool_display::{
            ToolDisplayLine, first_text_after_thinking_block, format_tool_execution_summary,
            format_tool_inline_line, format_tool_start_line, tool_batch_complete,
            tool_batch_pending_count, tool_batch_redraw_cursor_up, tool_line_clear_prefix,
        },
        agents::{
            AgentConfig, AgentConfigOptions, AgentsCancelToken,
            hooks::{build_hook_registry, wrap_tools_with_hooks},
        },
        cli::Cli,
        config::{Config, McpConfig},
        config_paths::resolved_config_path,
        providers::Provider,
        repl::{
            prompt_symbol,
            startup_hints::{StartupHintInput, StartupHintPart, startup_hint_parts},
        },
        session::{
            AgentId, FocusedSession, SessionId, SessionManager, SessionRuntimeStatus,
            spawn_session_root, sync_session_meta, teardown_session,
        },
        setup,
        stdout::StdoutMessage,
        tokio::AppCancelToken,
        utils::format_usage_line,
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
        platform::collections::HashMap,
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
    /// From yoagent `ModelConfig.context_window` at install — used by `/tokens` max line.
    context_window: u32,
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

    pub fn context_window(&self) -> u32 {
        self.context_window
    }

    fn bind_session(&mut self, session_id: SessionId) {
        self.session_id = session_id;
    }

    pub async fn new_with_agent_config(agent_config: &AgentConfig) -> Self {
        let AgentConfig {
            model,
            skills,
            system_prompt,
            api_key,
            mcp,
            ..
        } = agent_config;

        let model_config = model_config_for(agent_config);
        let context_window = model_config.context_window;
        let agent = match agent_config.provider {
            Provider::Anthropic => Agent::new(AnthropicProvider),
            Provider::Cerebras
            | Provider::DeepSeek
            | Provider::Groq
            | Provider::MiniMax
            | Provider::Mistral
            | Provider::OpenAi
            | Provider::OpenRouter
            | Provider::Xai
            | Provider::Zai
            | Provider::Custom => Agent::new(OpenAiCompatProvider),
            Provider::Google => Agent::new(GoogleProvider),
        };

        let tools = {
            let base = default_tools();
            if agent_config.shell_hooks.is_empty() {
                base
            } else {
                let registry = build_hook_registry(&agent_config.shell_hooks);
                wrap_tools_with_hooks(base, &registry)
            }
        };

        let mut agent = agent
            .with_model_config(model_config)
            .with_system_prompt(system_prompt)
            .with_model(model)
            .with_api_key(api_key)
            .with_tools(tools);

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
            context_window,
        }
    }
}

/// yoagent `ModelConfig` for the active provider — single source for context window at install.
pub(crate) fn model_config_for(agent_config: &AgentConfig) -> ModelConfig {
    let AgentConfig {
        model,
        provider,
        base_url,
        ..
    } = agent_config;

    match provider {
        Provider::Anthropic => ModelConfig::anthropic(model, model),
        Provider::Cerebras => ModelConfig::openai(model, model),
        Provider::Custom => ModelConfig::local(base_url, model),
        Provider::DeepSeek => ModelConfig::deepseek(model, model),
        Provider::Google => ModelConfig::google(model, model),
        Provider::Groq => ModelConfig::groq(model, model),
        Provider::MiniMax => ModelConfig::minimax(model, model),
        Provider::Mistral => ModelConfig::mistral(model, model),
        Provider::OpenAi => ModelConfig::openai(model, model),
        Provider::OpenRouter => ModelConfig::openai(model, model),
        Provider::Xai => ModelConfig::xai(model, model),
        Provider::Zai => ModelConfig::zai(model, model),
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
    () = coding_agent
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

/// Loopback BRP `session.clear` enqueue target; drained by the REPL stdin loop.
#[derive(Clone, Resource)]
pub struct CodingAgentClearChannel {
    pub sender: crossbeam_channel::Sender<()>,
    pub receiver: crossbeam_channel::Receiver<()>,
}

impl Default for CodingAgentClearChannel {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { sender, receiver }
    }
}

impl CodingAgentClearChannel {
    pub fn request_clear(&self) {
        let _ = self.sender.send(());
    }
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
    let opts = AgentConfigOptions::from_cli(cli.as_ref());
    let agent_config = AgentConfig::from_config(&config, opts);
    let model = agent_config.model.clone();
    let provider = agent_config.provider.to_string();

    tokio_runtime.spawn_background_task(move |mut ctx| async move {
        let agent_config = AgentConfig::from_config(&config, opts);
        let coding_agent = CodingAgent::new_with_agent_config(&agent_config).await;
        ctx.run_on_main_thread(move |ctx| {
            () = install_coding_agent(ctx.world, coding_agent, model, provider);
        })
        .await;
    });

    () = commands.init_resource::<CodingAgentPromptChannel>();
    () = commands.init_resource::<CodingAgentClearChannel>();

    let app_cancel = app_cancel.clone();
    let agents_cancel = agents_cancel.clone();

    tokio_runtime.spawn_background_task(|_ctx| async move {
        tokio::select! {
            _ = app_cancel.cancelled() => (),
            _ = agents_cancel.cancelled() => (),
            else => unreachable!(),
        }
    });

    if let Ok(cwd) = env::current_dir() {
        let config_path = resolved_config_path(&cwd);
        let hint_input = StartupHintInput {
            bare: cli.bare,
            no_hints: cli.no_hints,
            print_system_prompt: cli.print_system_prompt,
            cwd: &cwd,
            config_path: &config_path,
            model: &agent_config.model,
            skills_len: agent_config.skills.len(),
            mcp_len: agent_config.mcp.len(),
            hooks_len: agent_config.shell_hooks.len(),
            needs_setup: setup::needs_setup(),
        };
        for part in startup_hint_parts(&hint_input) {
            match part {
                StartupHintPart::Banner(line) => stdout.write(StdoutMessage::from(line)),
                StartupHintPart::Dimmed(line) => stdout.write(StdoutMessage::from(line.dimmed())),
            };
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
    /// Active parallel tool rows (cleared after the batch fully completes).
    tool_batch_display: Vec<ToolDisplayLine>,
    /// `tool_call_id` → index in `tool_batch_display`.
    tool_batch_index_by_id: HashMap<String, usize>,
    /// Whether the last thinking delta already ended with `\n` (avoids blank line before divider).
    thinking_trailing_newline: bool,
    /// Strip leading `\n` from the next non-empty text delta (after thinking divider gap).
    pending_text_after_thinking_gap: bool,
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

/// BRP `session.clear` and headless dev_native drain target (REPL `/clear!` reinstalls directly).
fn drain_session_clear_requests(
    clear_channel: Res<CodingAgentClearChannel>,
    agent_config: Res<AgentConfig>,
    mut tokio_runtime: ResMut<TokioTasksRuntime>,
) {
    while clear_channel.receiver.try_recv().is_ok() {
        let config = agent_config.clone();
        let model = config.model.clone();
        let provider = config.provider.to_string();
        tokio_runtime.spawn_background_task(move |mut ctx| async move {
            let coding_agent = CodingAgent::new_with_agent_config(&config).await;
            ctx.run_on_main_thread(move |main_ctx| {
                () = install_coding_agent(main_ctx.world, coding_agent, model, provider);
            })
            .await;
        });
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

            // yoagent updates `messages()` only after `finish()`; without this,
            // `/tokens` and other readers see pre-turn state until the next prompt.
            {
                let mut agent = coding_agent.lock().await;
                () = agent.finish().await;
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

                () = world
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

fn write_tool_batch_redraw(stdout: &mut MessageWriter<StdoutMessage>, lines: &[ToolDisplayLine]) {
    if lines.is_empty() {
        return;
    }
    let cursor_up = tool_batch_redraw_cursor_up(lines.len());
    if !cursor_up.is_empty() {
        stdout.write(StdoutMessage::from(cursor_up));
    }
    for (idx, line) in lines.iter().enumerate() {
        stdout.write(StdoutMessage::from(tool_line_clear_prefix()));
        stdout.write(StdoutMessage::from(<&str as Colorize>::yellow(
            format_tool_inline_line(&line.summary).as_str(),
        )));
        if let Some(is_error) = line.finished {
            let symbol = if is_error {
                <&str as Colorize>::red(" ✗")
            } else {
                <&str as Colorize>::green(" ✓")
            };
            stdout.write(StdoutMessage::from(symbol));
        }
        if idx < lines.len().saturating_sub(1) {
            stdout.write(StdoutMessage::newline());
        }
    }
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
            tool_batch_display,
            tool_batch_index_by_id,
            thinking_trailing_newline,
            pending_text_after_thinking_gap,
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
                if let AgentMessage::Llm(llm_message) = &message
                    && let Some(line) = assistant_error_line(llm_message)
                        .or_else(|| empty_assistant_hint(llm_message))
                {
                    stdout.write(StdoutMessage::from(line));
                }
            }
            AgentEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
                args,
                ..
            } => {
                if *in_thinking {
                    if !cli.no_hints {
                        if !*thinking_trailing_newline {
                            stdout.write(StdoutMessage::newline());
                        }
                        stdout.write(thinking_divider());
                    }
                    *in_thinking = false;
                }
                if *in_text {
                    *in_text = false;
                }
                let summary = format_tool_execution_summary(tool_name, args);
                let batch_idx = tool_batch_display.len();
                tool_batch_index_by_id.insert(tool_call_id.clone(), batch_idx);
                () = tool_batch_display.push(ToolDisplayLine {
                    summary: summary.clone(),
                    finished: None,
                });
                if !cli.no_hints {
                    let line = format_tool_start_line(&summary);
                    stdout.write(StdoutMessage::from(<&str as Colorize>::yellow(
                        line.as_str(),
                    )));
                }
            }
            AgentEvent::ToolExecutionEnd {
                tool_call_id,
                is_error,
                ..
            } if !cli.no_hints => {
                if let Some(&batch_idx) = tool_batch_index_by_id.get(tool_call_id)
                    && let Some(line) = tool_batch_display.get_mut(batch_idx)
                {
                    line.finished = Some(*is_error);
                }
                () = write_tool_batch_redraw(&mut stdout, tool_batch_display);
                if tool_batch_complete(tool_batch_pending_count(tool_batch_display)) {
                    stdout.write(StdoutMessage::newline());
                    () = tool_batch_display.clear();
                    () = tool_batch_index_by_id.clear();
                }
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Text { delta },
                ..
            } => {
                // Exit thinking block when the first normal text delta arrives.
                if *in_thinking {
                    if !cli.no_hints {
                        if !*thinking_trailing_newline {
                            stdout.write(StdoutMessage::newline());
                        }
                        stdout.write(thinking_divider());
                    }
                    *in_thinking = false;
                    *pending_text_after_thinking_gap = true;
                }

                let after_thinking_gap = *pending_text_after_thinking_gap;
                if !*in_text {
                    if !cli.no_hints && !after_thinking_gap {
                        stdout.write(StdoutMessage::newline());
                    }
                    *in_text = true;
                }
                if let Some(text) =
                    first_text_after_thinking_block(delta, pending_text_after_thinking_gap)
                {
                    let out = if after_thinking_gap {
                        format!("\n{text}")
                    } else {
                        String::from(text)
                    };
                    stdout.write(StdoutMessage::from(out));
                }
            }
            AgentEvent::MessageUpdate { delta, .. } if !cli.no_hints => {
                if let Some(thinking_text) = extract_thinking_from_delta(delta) {
                    if thinking_text.trim().is_empty() {
                        // Ignore empty thinking deltas to avoid blank thinking blocks.
                        continue;
                    }

                    if !*in_thinking {
                        // Advance past the tool batch gap line, or break off an open tool line.
                        stdout.write(StdoutMessage::newline());
                        *thinking_trailing_newline = false;
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
                    *thinking_trailing_newline = thinking_text.ends_with('\n');
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
                    stdout.write(StdoutMessage::newline());
                    stdout.write(thinking_header());
                    stdout.write(StdoutMessage::from(thinking_text.dimmed()));
                    if !thinking_text.ends_with('\n') {
                        stdout.write(StdoutMessage::newline());
                    }
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
                () = tool_batch_display.clear();
                () = tool_batch_index_by_id.clear();
                *thinking_trailing_newline = false;
                *pending_text_after_thinking_gap = false;

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
                            if !cli.no_hints {
                                stdout.write(StdoutMessage::from(usage_info(usage)));
                            } else {
                                stdout.write(StdoutMessage::newline());
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
                (
                    drain_session_clear_requests,
                    spawn_agent_task,
                )
                    .chain()
                    .run_if(
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
