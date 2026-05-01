use {
    crate::{
        agents::{AgentsCancelToken, AgentConfig},
        cli::Cli,
        tokio::AppCancelToken,
        tui::TuiMain,
        utils::truncate,
    },
    ansi_to_tui::IntoText,
    bevy::{
        app::{App, AppExit, PostUpdate, Startup, Update},
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            message::{Message, MessageReader},
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
    ratatui::{
        style::Stylize,
        text::{Line, Span},
    },
    std::{
        io::{Write, stdout},
        sync::Arc,
    },
    termimad::MadSkin,
    tokio::sync::Mutex,
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
    agent_config: Res<AgentConfig>,
    cli: Res<Cli>,
    mut tui: Option<NonSendMut<TuiMain>>,
    mut commands: Commands,
    app_cancel: Res<AppCancelToken>,
    agents_cancel: Res<AgentsCancelToken>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
) {
    if let Some(tui) = tui.as_mut() {
        tui.output.push(Line::<'_>::from(
            "🚀 Starting Interactive Coding Agent Session",
        ));
    }

    let AgentConfig {
        base_url,
        model,
        api_key,
    } = agent_config.into_inner();
    let cli = cli.into_inner();

    let model_config = ModelConfig::local(base_url, model);
    let mut agent = Agent::new(OpenAiCompatProvider)
        .with_model_config(model_config)
        .with_system_prompt(SYSTEM_PROMPT)
        .with_model(model)
        .with_api_key(api_key)
        .with_tools(default_tools());

    if let Some(skills) = cli.skills.clone() {
        let skills = SkillSet::load(skills.as_slice()).expect("Failed to load skills");
        agent = agent.with_skills(skills);
    }

    let coding_agent = CodingAgent(Arc::new(Mutex::new(agent)));

    commands.insert_resource::<CodingAgent>(coding_agent);
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

#[derive(Debug, Deref, DerefMut, Message)]
pub struct CodingAgentEvent(AgentEvent);

fn spawn_agent_task(
    channel: Res<CodingAgentPromptChannel>,
    runtime: ResMut<TokioTasksRuntime>,
    coding_agent: ResMut<CodingAgent>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<CodingAgentState>>,
) {
    if let Ok(prompt) = channel.receiver.try_recv() {
        let coding_agent = coding_agent.clone();
        runtime.spawn_background_task(move |mut ctx| async move {
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

fn handle_coding_agent_events(
    mut messages: MessageReader<CodingAgentEvent>,
    mut coding_agent_task: ResMut<CodingAgentTask>,
    mut tui: Option<NonSendMut<TuiMain>>,
) {
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
                if *in_text {
                    if let Some(tui) = tui.as_mut() {
                        let span: Span<'_> =
                            format!("🔧 Tool Call: {tool_name} with args: {args}").yellow();
                        tui.output.push(Line::from(span));
                        tui.output.push(Line::from(""));
                    }
                    *in_text = false;
                }
                let summary = match tool_name.as_str() {
                    "bash" => {
                        let cmd = args
                            .get("command")
                            .and_then(|v| v.as_str())
                            .unwrap_or("...");
                        format!("$ {}", truncate(cmd, 60))
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

                if let Some(tui) = tui.as_mut() {
                    if let Some(line) = tui.output.last_mut() {
                        *line = Line::from(summary).yellow();
                    }

                    tui.scroll_to_bottom();
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
                    tui.output.push(line);
                    tui.scroll_to_bottom();
                }
            }
            AgentEvent::MessageUpdate {
                delta: StreamDelta::Text { delta },
                ..
            } => {
                if !*in_text {
                    buf.clear();
                    if let Some(tui) = tui.as_mut() {
                        tui.output.push(Line::from(""));
                        let line = Line::from("📝 Agent Response:");
                        tui.output.push(line);
                        let span = "─".repeat(50).blue();
                        let line = Line::from(span);
                        tui.output.push(line);
                        tui.output.push(Line::from(""));
                    }
                    *tui_output_index = 0;
                    *in_text = true;
                }

                if tui.is_none() {
                    print!("{delta}");
                    stdout().flush().unwrap();
                } else if let Some(tui) = tui.as_mut() {
                    buf.push_str(delta);

                    let skin = MadSkin::default();

                    let output_len = tui.output.len();
                    tui.output.truncate(output_len - 1 - *tui_output_index);

                    let mut out = String::new();
                    skin.write_text_on(unsafe { out.as_mut_vec() }, buf)
                        .unwrap();

                    let text = out.into_text().unwrap();
                    *tui_output_index = text.lines.len();
                    for line in text.lines {
                        tui.output.push(line);
                    }

                    let span = "─".repeat(50).blue();
                    let line = Line::from(span);
                    tui.output.push(line);

                    tui.scroll_to_bottom();
                }
            }
            AgentEvent::AgentEnd { messages } => {
                for msg in messages.iter().rev() {
                    if let AgentMessage::Llm(yoagent::types::Message::Assistant { usage, .. }) = msg
                    {
                        *last_usage = usage.clone();
                        break;
                    }
                }
            }
            _ => (),
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
    app.add_systems(Startup, setup)
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
