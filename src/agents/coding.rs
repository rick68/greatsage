use {
    crate::{
        Args,
        agents::{AgentsCancelToken, LlmConfig, PermissionConfig, retry_async},
        tokio::AppCancelToken,
        tui::TuiMain,
    },
    ansi_to_tui::IntoText,
    bevy::{
        app::{App, AppExit, PostUpdate, Startup, Update},
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            message::{Message, MessageId, MessageReader},
            resource::Resource,
            schedule::{
                IntoScheduleConfigs, NotSystem, ScheduleConfigTupleMarker, SystemCondition,
                common_conditions::{not, resource_exists},
            },
            system::{Commands, FunctionSystem, IsFunctionSystem},
            world::World,
        },
        prelude::{Deref, DerefMut},
        state::{
            app::AppExtStates,
            condition::in_state,
            state::{NextState, States},
        },
    },
    bevy_tokio_tasks::{MainThreadContext, TaskContext, TokioTasksRuntime},
    ratatui::{
        style::Stylize,
        text::{Line, Span, Text},
    },
    serde_json::Value,
    std::{
        io::{Write, stdout},
        sync::Arc,
    },
    termimad::MadSkin,
    tokio::{
        sync::{Mutex, mpsc::UnboundedReceiver},
        task::JoinHandle,
    },
    tokio_util::sync::CancellationToken,
    yoagent::{
        agent::Agent,
        provider::{ModelConfig, openai_compat::OpenAiCompatProvider},
        skills::SkillSet,
        tools::default_tools,
        types::{AgentEvent, AgentMessage, StreamDelta, Usage},
    },
};

const SYSTEM_PROMPT: &str = r#"You are a coding assistant working in the user's terminal.
You have access to the filesystem and shell. Be direct and concise.
When the user asks you to do something, do it — don't just explain how.
Use tools proactively: read files to understand context, run commands to verify your work.
After making changes, run tests or verify the result when appropriate."#;

#[derive(Deref, DerefMut, Resource)]
struct CodingAgent(Arc<Mutex<Agent>>);

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

fn setup(
    llm_config: Res<'_, LlmConfig>,
    args: Res<'_, Args>,
    mut tui: Option<NonSendMut<'_, TuiMain<'_>>>,
    mut commands: Commands<'_, '_>,
    app_cancel: Res<'_, AppCancelToken>,
    agents_cancel: Res<'_, AgentsCancelToken>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
) {
    if let Some(tui) = tui.as_mut() {
        () = tui.output.push(Line::<'_>::from(
            "🚀 Starting Interactive Coding Agent Session",
        ));
    }

    let LlmConfig {
        base_url,
        model,
        api_key,
    } = llm_config.into_inner();
    let args: &Args = args.into_inner();

    let model_config: ModelConfig = ModelConfig::local(base_url, model);
    let mut agent: Agent = Agent::new(OpenAiCompatProvider)
        .with_model_config(model_config)
        .with_system_prompt(SYSTEM_PROMPT)
        .with_model(model)
        .with_api_key(api_key)
        .with_tools(default_tools());

    if let Some(skills) = args.skills.clone() {
        let skills: SkillSet = SkillSet::load(skills.as_slice()).expect("Failed to load skills");
        agent = agent.with_skills(skills);
    }

    let coding_agent: CodingAgent = CodingAgent(Arc::new(Mutex::new(agent)));

    () = commands.insert_resource::<CodingAgent>(coding_agent);
    () = commands.init_resource::<CodingAgentPromptChannel>();
    () = commands.init_resource::<TokenUsage>();

    let app_cancel: Arc<CancellationToken> = app_cancel.clone();
    let agents_cancel: Arc<CancellationToken> = agents_cancel.clone();

    let _: JoinHandle<()> =
        tokio_runtime.spawn_background_task::<_, (), _>(|_ctx: TaskContext| async move {
            tokio::select! {
                _ = app_cancel.cancelled() => (),
                _ = agents_cancel.cancelled() => (),
                else => unreachable!(),
            }
        });
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
    tui_output_index: usize,
    buffer: String,
}
#[derive(Default, Resource)]
pub struct TokenUsage(pub Usage);

#[derive(Debug, Deref, DerefMut, Message)]
pub struct CodingAgentEvent(AgentEvent);

fn spawn_agent_task(
    channel: Res<'_, CodingAgentPromptChannel>,
    runtime: ResMut<'_, TokioTasksRuntime>,
    coding_agent: ResMut<'_, CodingAgent>,
    mut commands: Commands<'_, '_>,
    mut next_state: ResMut<'_, NextState<CodingAgentState>>,
) {
    if let Ok(prompt) = channel.receiver.try_recv() {
        let coding_agent: Arc<Mutex<Agent>> = coding_agent.clone();
        let _: JoinHandle<()> =
            runtime.spawn_background_task::<_, (), _>(move |mut ctx: TaskContext| async move {
                // Try to prompt the LLM with retry logic.
                let rx_result: Result<UnboundedReceiver<AgentEvent>, ()> = retry_async(|| async {
                    // The prompt itself does not return a Result, so we wrap it.
                    Ok(coding_agent.lock().await.prompt(prompt.clone()).await)
                })
                .await;
                match rx_result {
                    Ok(mut rx) => {
                        while let Some(event) = rx.recv().await {
                            () = ctx
                                .run_on_main_thread::<_, ()>(|ctx: MainThreadContext<'_>| {
                                    let _: Option<MessageId<CodingAgentEvent>> =
                                        ctx.world.write_message::<CodingAgentEvent>(
                                            CodingAgentEvent(event),
                                        );
                                })
                                .await;
                        }
                        // When done, reset state to Idle
                        () = ctx
                            .run_on_main_thread::<_, ()>(|ctx: MainThreadContext<'_>| {
                                let world: &mut World = ctx.world;
                                let _: Option<CodingAgentTask> =
                                    world.remove_resource::<CodingAgentTask>();
                                () = world
                                    .get_resource_mut::<NextState<CodingAgentState>>()
                                    .unwrap()
                                    .set(CodingAgentState::Idle);
                            })
                            .await;
                    }
                    Err(_e) => {
                        () = ctx
                            .run_on_main_thread::<_, ()>(|ctx: MainThreadContext<'_>| {
                                let err_log: &str = "Error: LLM request failed";

                                let world: &mut World = ctx.world;
                                // Log error to TUI output if available, then set state Idle.
                                if let Some(mut tui) =
                                    world.get_non_send_resource_mut::<NonSendMut<'_, TuiMain<'_>>>()
                                {
                                    () = tui
                                        .output
                                        .push(Line::<'_>::from(String::from(err_log).red()));
                                } else {
                                    // Since we don't have direct TUI access here, we simply print.
                                    eprintln!("{err_log}");
                                }

                                let _: Option<CodingAgentTask> =
                                    world.remove_resource::<CodingAgentTask>();
                                () = world
                                    .get_resource_mut::<NextState<CodingAgentState>>()
                                    .unwrap()
                                    .set(CodingAgentState::Idle);
                            })
                            .await;
                    }
                }

                // The event processing and cleanup is handled inside the match arms above.
            });

        () = commands.init_resource::<CodingAgentTask>();
        () = next_state.set(CodingAgentState::Processing);
    }
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

fn handle_coding_agent_events(
    mut messages: MessageReader<'_, '_, CodingAgentEvent>,
    mut coding_agent_task: ResMut<'_, CodingAgentTask>,
    mut token_usage: ResMut<'_, TokenUsage>,
    mut tui: Option<NonSendMut<'_, TuiMain<'_>>>,
    permission: Res<'_, PermissionConfig>,
) {
    // let tui: &mut TuiMain<'_> = tui.into_inner();

    for CodingAgentEvent(event) in messages.read() {
        let CodingAgentTask {
            last_usage,
            in_text,
            tui_output_index,
            buffer: buf,
        } = coding_agent_task.as_mut();

        match event {
            AgentEvent::ToolExecutionStart {
                tool_name, args, ..
            } => {
                // Permission check for file system related tools
                let permission_check =
                    |path: &str| -> Result<(), String> { (*permission).validate_path(path) };
                let maybe_path: Option<&str> = match tool_name.as_str() {
                    "read_file" | "write_file" | "edit_file" | "list_files" => {
                        args.get::<&str>("path").and_then(|v| v.as_str())
                    }
                    "search" => args.get::<&str>("path").and_then(|v| v.as_str()),
                    "bash" => args.get::<&str>("command").and_then(|v| v.as_str()),
                    _ => None,
                };
                if let Some(p) = maybe_path
                    && let Err(msg) = permission_check(p)
                {
                    if let Some(tui) = tui.as_mut() {
                        () = tui.output.push(Line::from(msg).red());
                        () = tui.scroll_to_bottom();
                    } else {
                        eprintln!("{}", msg);
                    }
                }
                if *in_text {
                    if let Some(tui) = tui.as_mut() {
                        let span: Span<'_> =
                            format!("🔧 Tool Call: {tool_name} with args: {args}").yellow();
                        () = tui.output.push(Line::<'_>::from(span));
                        () = tui.output.push(Line::<'_>::from(""));
                    }
                    *in_text = false;
                }
                let summary: String = match tool_name.as_str() {
                    "bash" => {
                        let cmd: &str = args
                            .get::<&str>("command")
                            .and_then::<&str, fn(&Value) -> Option<&str>>(
                                |v: &Value| -> Option<&str> { v.as_str() },
                            )
                            .unwrap_or("...");
                        format!("$ {}", truncate(cmd, 60))
                    }
                    "read_file" => {
                        let path: &str = args
                            .get::<&str>("path")
                            .and_then::<&str, fn(&Value) -> Option<&str>>(
                                |v: &Value| -> Option<&str> { v.as_str() },
                            )
                            .unwrap_or("?");
                        format!("read {path}")
                    }
                    "write_file" => {
                        let path: &str = args
                            .get::<&str>("path")
                            .and_then::<&str, fn(&Value) -> Option<&str>>(
                                |v: &Value| -> Option<&str> { v.as_str() },
                            )
                            .unwrap_or("?");
                        format!("write {path}")
                    }
                    "edit_file" => {
                        let path: &str = args
                            .get::<&str>("path")
                            .and_then::<&str, fn(&Value) -> Option<&str>>(
                                |v: &Value| -> Option<&str> { v.as_str() },
                            )
                            .unwrap_or("?");
                        format!("edit {path}")
                    }
                    "list_files" => {
                        let path: &str = args
                            .get::<&str>("path")
                            .and_then::<&str, fn(&Value) -> Option<&str>>(
                                |v: &Value| -> Option<&str> { v.as_str() },
                            )
                            .unwrap_or(".");
                        format!("ls {path}")
                    }
                    "search" => {
                        let pat = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
                        format!("search '{}'", truncate(pat, 60))
                    }
                    _ => tool_name.clone(),
                };

                if let Some(tui) = tui.as_mut() {
                    if let Some(line) = tui.output.last_mut() {
                        *line = Line::from(summary).yellow();
                    }

                    () = tui.scroll_to_bottom();
                }
            }
            AgentEvent::ToolExecutionEnd {
                tool_name,
                result,
                is_error,
                ..
            } => {
                let line: Line<'_> = if *is_error {
                    Line::from(format!(
                        "❌ Tool Incompleted: {tool_name} - Result: {result:?}"
                    ))
                    .red()
                } else {
                    Line::from(format!(
                        "✅ Tool Completed: {tool_name} - Result: {result:?}"
                    ))
                    .yellow()
                };

                if let Some(tui) = tui.as_mut() {
                    () = tui.output.push(line);
                    () = tui.scroll_to_bottom();
                }
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Text { delta },
                ..
            } => {
                if !*in_text {
                    () = buf.clear();
                    if let Some(tui) = tui.as_mut() {
                        () = tui.output.push(Line::<'_>::from(""));
                        let line: Line<'_> = Line::<'_>::from("📝 Agent Response:");
                        () = tui.output.push(line);
                        let span: Span<'_> = "─".repeat(50).blue();
                        let line: Line<'_> = Line::<'_>::from(span);
                        () = tui.output.push(line);
                        () = tui.output.push(Line::<'_>::from(""));
                    }
                    *tui_output_index = 0;
                    *in_text = true;
                }

                if tui.is_none() {
                    print!("{delta}");
                    () = stdout().flush().unwrap();
                } else if let Some(tui) = tui.as_mut() {
                    () = buf.push_str(delta);

                    let skin: MadSkin = MadSkin::default();

                    let output_len: usize = tui.output.len();
                    () = tui.output.truncate(output_len - 1 - *tui_output_index);

                    let mut out: String = String::new();
                    () = skin
                        .write_text_on::<Vec<u8>>(unsafe { out.as_mut_vec() }, buf)
                        .unwrap();

                    let text: Text<'_> = out.into_text().unwrap();
                    *tui_output_index = text.lines.len();
                    for line in text.lines {
                        () = tui.output.push(line);
                    }

                    let span: Span<'_> = "─".repeat(50).blue();
                    let line: Line<'_> = Line::<'_>::from(span);
                    () = tui.output.push(line);

                    () = tui.scroll_to_bottom();
                }
            }
            AgentEvent::AgentEnd { messages } => {
                for msg in messages.iter().rev() {
                    if let AgentMessage::Llm(yoagent::types::Message::Assistant { usage, .. }) = msg
                    {
                        *last_usage = usage.clone();
                        token_usage.0 = usage.clone();
                        break;
                    }
                }
            }
            _ => (),
        }
    }
}

fn shutdown_coding_agent(
    mut messages: MessageReader<'_, '_, AppExit>,
    mut cancel: Option<Res<'_, AgentsCancelToken>>,
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
    let _: &mut App =
        app.add_systems::<_>(Startup, setup)
            .init_state::<CodingAgentState>()
            .add_message::<CodingAgentEvent>()
            .add_systems::<(ScheduleConfigTupleMarker, (), ())>(
                Update,
                (
                    spawn_agent_task.run_if::<()>(
                        in_state::<CodingAgentState>(CodingAgentState::Idle).and::<(), NotSystem<
                            FunctionSystem<
                                fn(
                                    Option<
                                        _, // Res<'_, CodingAgentTask>
                                    >,
                                ) -> bool,
                                (),
                                bool,
                                fn(Option<Res<'_, CodingAgentTask>>) -> bool,
                            >,
                        >>(
                            not::<
                                (
                                    IsFunctionSystem,
                                    fn(
                                        Option<
                                            _, // Res<'_, CodingAgentTask>
                                        >,
                                    ) -> bool,
                                ),
                                bool,
                                fn(Option<Res<'_, CodingAgentTask>>) -> bool,
                            >(resource_exists::<CodingAgentTask>),
                        ),
                    ),
                    handle_coding_agent_events.run_if::<()>(
                        in_state::<CodingAgentState>(CodingAgentState::Processing).and::<(
                            IsFunctionSystem,
                            fn(
                                Option<
                                    _, // Res<'_, CodingAgentTask>
                                >,
                            ) -> bool,
                        ), fn(
                            Option<Res<'_, CodingAgentTask>>,
                        )
                            -> bool>(
                            resource_exists::<CodingAgentTask>,
                        ),
                    ),
                ),
            )
            .add_systems::<(
                IsFunctionSystem,
                fn(
                    _, // MessageReader<'_, '_, AppExit>
                    _, // Option<Res<'_, AgentsCancelToken>>
                ) -> (),
            )>(PostUpdate, shutdown_coding_agent);
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use {super::*, pretty_assertions::assert_eq};

    #[test]
    fn truncates_short_string() {
        let s: &str = "Hello";
        assert_eq!(truncate(s, 10), "Hello");
    }

    #[test]
    fn truncates_exact_length() {
        let s: &str = "Hello";
        assert_eq!(truncate(s, 5), "Hello");
    }

    #[test]
    fn truncates_unicode_without_splitting() {
        let s: &str = "🦀Rust";
        // The crab emoji is a single Unicode scalar value.
        assert_eq!(truncate(s, 1), "🦀");
    }
}
