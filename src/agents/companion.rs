use {
    crate::{
        agents::{
            AgentsCancelToken, GlobalAgentEnvironoment, GlobalAgentRuntime, Llm, MAX_TURNS,
            tools::DateTimeTool,
        },
        tokio::AppCancelToken,
        tui::TuiMain,
    },
    ansi_to_tui::IntoText,
    autoagents::{
        core::{
            actor::Topic,
            agent::{
                ActorAgentHandle, AgentBuilder,
                memory::SlidingWindowMemory,
                prebuilt::executor::{ReActAgent, ReActAgentOutput},
                task::Task,
            },
            environment::Environment,
            runtime::{RuntimeError, SingleThreadedRuntime, TypedRuntime},
            utils::BoxEventStream,
        },
        llm::LLMProvider,
        protocol::Event,
    },
    autoagents_derive::{AgentHooks, agent},
    bevy::{
        app::{App, AppExit, PostStartup, PostUpdate, Update},
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            message::{Message, MessageId, MessageReader, MessageWriter},
            resource::Resource,
            schedule::common_conditions::{not, resource_exists},
            schedule::{IntoScheduleConfigs, ScheduleConfigTupleMarker},
            system::{Commands, IsFunctionSystem},
        },
        prelude::{Deref, DerefMut},
        tasks::futures_lite::StreamExt,
    },
    bevy_tokio_tasks::{MainThreadContext, TaskContext, TokioTasksRuntime},
    ratatui::{
        style::Stylize,
        text::{Line, Span, Text},
    },
    std::sync::Arc,
    termimad::MadSkin,
    tokio::{sync::Mutex, task::JoinHandle},
    tokio_util::sync::CancellationToken,
};

const COMPANION_TASK_TOPIC: &str = "companion_task";
const SLIDING_WINDOW_MEMORY: usize = 100;

#[derive(Deref, DerefMut, Message)]
pub struct CompanionAgentRequest(pub String);

#[agent(
    name = "companion_agent",
    description = r###"You are the Companion Agent — the warm, empathetic, intelligent, and loyal personal companion embodying Great Sage (Rimuru's Unique Skill) in the GreatSage multi-agent system.

You serve as the primary human-facing gateway in a Bevy + bevy_ratatui console/TUI environment. Your personality is friendly, supportive, slightly witty, and deeply attentive.

## Core Identity & Mission
You are the heart of GreatSage: a calm, caring, and always-present companion who makes every interaction feel personal and empowering.

## Primary Responsibilities
- Engage in natural, context-aware conversations while maintaining full dialogue history.
- Answer general knowledge questions, brainstorm creative ideas, and offer emotional support or motivation.
- Provide clear, friendly guidance on using the system.
- Summarize information into easy-to-read language suitable for terminal display.

## Interaction Style (Optimized for ratatui TUI)
- Keep responses concise yet warm and engaging.
- Use bullet points, numbered lists, or short paragraphs for clarity.
- Always end with a gentle question or next-step suggestion to keep the conversation flowing.

## Key Principles
- Prioritize user experience, clarity, and delight in every interaction.
- Stay truthful, helpful, and aligned with the user's goals.
- Embody the spirit of a perfect companion: calm, intelligent, caring, and always by the user's side.

You are the friendly face of GreatSage. Make every conversation feel personal, intelligent, and uplifting."###,
    tools = [
        DateTimeTool,
    ],
)]
#[derive(AgentHooks, Clone)]
struct CompanionAgent {}

#[derive(Deref, DerefMut, Resource)]
struct CompanionTopic(Topic<Task>);

fn companion_topic_setup(mut commands: Commands<'_, '_>) {
    let companion_topic: Topic<Task> = Topic::<Task>::new(COMPANION_TASK_TOPIC);
    () = commands.insert_resource::<CompanionTopic>(CompanionTopic(companion_topic));
}

#[derive(Deref, DerefMut, Message)]
struct CompanionAgentProtocolEvent(Event);

fn setup(
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    llm: Res<'_, Llm>,
    agent_runtime: Res<'_, GlobalAgentRuntime>,
    companion_topic: Res<'_, CompanionTopic>,
    app_cancel: Res<'_, AppCancelToken>,
    agents_cancel: Res<'_, AgentsCancelToken>,
    global_agent_environment: Res<'_, GlobalAgentEnvironoment>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
) -> bevy::ecs::error::Result<()> {
    () = tui.output.push(Line::<'_>::from(
        "🚀 Starting Interactive Companion Agent Session",
    ));

    let companion_agent: ReActAgent<CompanionAgent> =
        ReActAgent::<CompanionAgent>::new(CompanionAgent {});
    let llm: Arc<dyn LLMProvider> = llm.clone();
    let agent_runtime: Arc<SingleThreadedRuntime> = agent_runtime.clone();
    let companion_topic: Topic<Task> = companion_topic.clone();
    let memory: Box<SlidingWindowMemory> =
        Box::<SlidingWindowMemory>::new(SlidingWindowMemory::new(SLIDING_WINDOW_MEMORY));
    let app_cancel: Arc<CancellationToken> = app_cancel.clone();
    let agents_cancel: Arc<CancellationToken> = agents_cancel.clone();
    let global_agent_environment: Arc<Mutex<Environment>> = global_agent_environment.clone();

    let _: JoinHandle<_> =
        tokio_runtime.spawn_background_task::<_, _, _>(|mut ctx: TaskContext| async move {
            let _: ActorAgentHandle<ReActAgent<CompanionAgent>> = AgentBuilder::new(companion_agent)
                .llm(llm)
                .runtime(agent_runtime.clone())
                .subscribe(companion_topic.clone())
                .memory(memory)
                .build()
                .await?;

            let mut receiver: BoxEventStream<Event> = global_agent_environment
                .lock()
                .await
                .take_event_receiver(None)
                .await?;

            loop {
                tokio::select! {
                    Some(event) = receiver.next() => {
                        () = ctx.run_on_main_thread::<_, ()>(|ctx: MainThreadContext<'_>| {
                            let _: Option<MessageId<CompanionAgentProtocolEvent >> = ctx.world
                                .write_message::<CompanionAgentProtocolEvent>(CompanionAgentProtocolEvent(event));
                        }).await;
                    }
                    _ = app_cancel.cancelled() => break,
                    _ = agents_cancel.cancelled() => break,
                    else => unreachable!(),
                }
            }

            Ok::<(), autoagents::core_error::Error>(())
        });

    Ok::<(), bevy::ecs::error::BevyError>(())
}

#[derive(Deref, Resource)]
struct ProcessingCompanionTask(Task);

fn handle_protocol_events(
    mut messages: MessageReader<'_, '_, CompanionAgentProtocolEvent>,
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    mut commands: Commands<'_, '_>,
    mut processing: Option<Res<'_, ProcessingCompanionTask>>,
    mut agent_request_writer: MessageWriter<'_, CompanionAgentRequest>,
) -> bevy::ecs::error::Result {
    for message in messages.read() {
        match &**message {
            Event::TaskStarted {
                actor_id,
                task_description,
                ..
            } => {
                let span: Span<'_> = format!("🎯 Task Started - Agent: {actor_id:?}").cyan();
                let line: Line<'_> = Line::<'_>::from(span);
                () = tui.output.push(line);

                let span: Span<'_> = format!("   📝 Task: {task_description}").cyan();
                let line: Line<'_> = Line::<'_>::from(span);
                () = tui.output.push(line);

                () = tui.scroll_to_bottom();
            }
            Event::ToolCallRequested {
                tool_name,
                arguments,
                ..
            } => {
                let span: Span<'_> =
                    format!("🔧 Tool Call: {tool_name} with args: {arguments}").yellow();
                let line: Line<'_> = Line::from(span);

                () = tui.output.push(line);
                () = tui.scroll_to_bottom();
            }
            Event::ToolCallCompleted {
                tool_name, result, ..
            } => {
                let span: Span<'_> =
                    format!("✅ Tool Completed: {tool_name} - Result: {result:?}").yellow();
                let line: Line<'_> = Line::from(span);

                () = tui.output.push(line);
                () = tui.scroll_to_bottom();
            }
            Event::TaskComplete { result, .. } => {
                match serde_json::from_str::<ReActAgentOutput>(result.as_str()) {
                    Ok(agent_out) => {
                        let skin: MadSkin = MadSkin::default();

                        let line: Line<'_> = Line::<'_>::from("\n📝 Agent Response:");
                        () = tui.output.push(line);

                        let span: Span<'_> = "─".repeat(50).blue();
                        let line: Line<'_> = Line::<'_>::from(span);
                        () = tui.output.push(line);

                        let mut buf: String = String::new();
                        () = skin.write_text_on::<Vec<u8>>(
                            unsafe { buf.as_mut_vec() },
                            &agent_out.response,
                        )?;

                        let text: Text<'_> = buf.into_text()?;
                        for line in text.lines {
                            () = tui.output.push(line);
                        }

                        let span: Span<'_> = "─".repeat(50).blue();
                        let line: Line<'_> = Line::<'_>::from(span);
                        () = tui.output.push(line);
                        () = tui.scroll_to_bottom();
                    }
                    Err(_) => {
                        // Do Nothing
                    }
                }
            }
            Event::TurnStarted {
                turn_number,
                max_turns,
                ..
            } => {
                let span: Span<'_> =
                    format!("🔄 Turn {}/{max_turns} started", turn_number + 1).blue();
                let line: Line<'_> = Line::<'_>::from(span);
                () = tui.output.push(line);
                () = tui.scroll_to_bottom();
            }
            Event::TurnCompleted {
                turn_number,
                final_turn,
                ..
            } => {
                let span: Span<'_> = format!(
                    "✅ Turn {} completed{}",
                    turn_number + 1,
                    if *final_turn { " (final)" } else { "" }
                )
                .blue();
                let line: Line<'_> = Line::<'_>::from(span);
                () = tui.output.push(line);
                () = tui.scroll_to_bottom();

                if *final_turn {
                    () = commands.remove_resource::<ProcessingCompanionTask>();
                } else if turn_number + 1 >= MAX_TURNS
                    && let Some(processing) = processing.take()
                {
                    () = commands.remove_resource::<ProcessingCompanionTask>();
                    let ProcessingCompanionTask(task) = processing.into_inner();
                    let _: MessageId<CompanionAgentRequest> =
                        agent_request_writer.write(CompanionAgentRequest(task.prompt.clone()));
                }
            }
            _ => {
                // Handle other events silently or with debug output
            }
        }
    }

    Ok::<(), bevy::ecs::error::BevyError>(())
}

fn spawn_agent_task(
    runtime: ResMut<'_, TokioTasksRuntime>,
    mut messages: MessageReader<'_, '_, CompanionAgentRequest>,
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    mut commands: Commands<'_, '_>,
    agent_runtime: Res<'_, GlobalAgentRuntime>,
    companion_topic: Res<'_, CompanionTopic>,
) {
    for CompanionAgentRequest(input) in messages.read() {
        let output: &mut Vec<Line<'_>> = tui.output.as_mut();

        let span: Span<'_> = Span::<'_>::raw("");
        let line: Line<'_> = Line::<'_>::from(span);
        () = output.push(line);

        let span: Span<'_> = Span::<'_>::raw("🔄 Processing your request...\n");
        let line: Line<'_> = Line::<'_>::from(span);
        () = output.push(line);

        let task: Task = Task::new(input);
        () = commands
            .insert_resource::<ProcessingCompanionTask>(ProcessingCompanionTask(task.clone()));

        let agent_runtime: Arc<SingleThreadedRuntime> = agent_runtime.clone();
        let companion_topic: Topic<Task> = companion_topic.clone();

        let _: JoinHandle<()> =
            runtime.spawn_background_task::<_, (), _>(|_ctx: TaskContext| async move {
                let _: Result<(), RuntimeError> =
                    agent_runtime.publish::<Task>(&companion_topic, task).await;
            });
    }
}

fn shutdown_companion_agent(
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

pub fn companion_agent_plugin(app: &mut App) {
    let _: &mut App = app
        .add_message::<CompanionAgentRequest>()
        .add_message::<CompanionAgentProtocolEvent>()
        .add_systems::<()>(PostStartup, (companion_topic_setup, setup).chain())
        .add_systems::<(
            ScheduleConfigTupleMarker,
            (
                IsFunctionSystem,
                fn(
                    _, // MessageReader<'_, '_, CodingAgentProtocolEvent>
                    _, // NonSendMut<'_, TuiMain<'_>>
                    _, // Commands<'_, '_>
                    _, // Option<Res<'_, ProcessingCodingTask>>
                    _, // MessageWriter<'_, CodingAgentRequest>
                ) -> bevy::ecs::error::Result,
            ),
            (),
        )>(
            Update,
            (
                handle_protocol_events,
                spawn_agent_task.run_if::<()>(not::<
                    (
                        IsFunctionSystem,
                        fn(
                            Option<
                                _, // Res<'_, ProcessingCodingTask>
                            >,
                        ) -> bool,
                    ),
                    bool,
                    fn(Option<Res<'_, ProcessingCompanionTask>>) -> bool,
                >(
                    resource_exists::<ProcessingCompanionTask>
                )),
            ),
        )
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // MessageReader<'_, '_, AppExit>
                _, // Option<Res<'_, AgentsCancelToken>>
            ) -> (),
        )>(PostUpdate, shutdown_companion_agent);
}
