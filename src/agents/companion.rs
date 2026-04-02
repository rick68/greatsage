use {
    crate::{
        agents::{
            AgentsCancelToken, GlobalAgentRuntime, Llm, MAX_TURNS, ProtocolEvent,
            SharedSlidingWindowMemory, routing::RouteTo, tools::DateTimeTool,
        },
        tui::TuiMain,
    },
    ansi_to_tui::IntoText,
    autoagents::{
        core::{
            actor::Topic,
            agent::{
                ActorAgentHandle, AgentBuilder,
                prebuilt::executor::{ReActAgent, ReActAgentOutput},
                task::Task,
            },
            runtime::{RuntimeError, SingleThreadedRuntime, TypedRuntime},
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
            schedule::IntoScheduleConfigs,
            schedule::common_conditions::{not, resource_exists},
            system::{Commands, IsFunctionSystem},
        },
        prelude::{Deref, DerefMut},
        state::condition::in_state,
    },
    bevy_tokio_tasks::{TaskContext, TokioTasksRuntime},
    ratatui::{
        style::Stylize,
        text::{Line, Span, Text},
    },
    std::sync::Arc,
    termimad::MadSkin,
    tokio::task::JoinHandle,
};

const COMPANION_TASK_TOPIC: &str = "companion_task";

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
pub struct CompanionAgentProtocolEvent(Event);

fn setup(
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    llm: Res<'_, Llm>,
    agent_runtime: Res<'_, GlobalAgentRuntime>,
    companion_topic: Res<'_, CompanionTopic>,
    shared_memory: Res<'_, SharedSlidingWindowMemory>,
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
    let shared_memory: Box<SharedSlidingWindowMemory> = Box::new(shared_memory.clone());

    let _: JoinHandle<Result<(), autoagents::core_error::Error>> = tokio_runtime
        .spawn_background_task::<_, _, _>(|_ctx: TaskContext| async move {
            let _: ActorAgentHandle<ReActAgent<CompanionAgent>> =
                AgentBuilder::new(companion_agent)
                    .llm(llm)
                    .runtime(agent_runtime.clone())
                    .subscribe(companion_topic.clone())
                    .memory(shared_memory)
                    .build()
                    .await?;

            Ok::<(), autoagents::core_error::Error>(())
        });

    Ok::<(), bevy::ecs::error::BevyError>(())
}

#[derive(Deref, Resource)]
struct ProcessingCompanionTask(Task);

fn handle_protocol_events(
    mut messages: MessageReader<'_, '_, ProtocolEvent>,
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
                () = commands.remove_resource::<ProcessingCompanionTask>();
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
        .add_systems::<()>(
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
            )
                .run_if::<(
                    IsFunctionSystem,
                    fn(
                        Option<
                            _, // Res<'_, RouteTo>
                        >,
                    ) -> bool,
                )>(in_state::<RouteTo>(RouteTo::Companion)),
        )
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // MessageReader<'_, '_, AppExit>
                _, // Option<Res<'_, AgentsCancelToken>>
            ) -> (),
        )>(PostUpdate, shutdown_companion_agent);
}
