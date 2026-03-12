mod coding;
mod tools;

use {
    self::coding::CodingAgent,
    crate::{tokio::AppCancelToken, tui::TuiMain},
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
        llm::{LLMProvider, backends::openai::OpenAI, builder::LLMBuilder},
        llm_error::LLMError,
        protocol::Event,
    },
    bevy::{
        app::{App, AppExit, Startup, Update},
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            error::BevyError,
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
    tokio::task::JoinHandle,
    tokio_util::sync::CancellationToken,
};

const MAX_TOKENS: u32 = 131_072;
const CODING_TASK_TOPIC: &str = "coding_task";
const MAX_TURNS: usize = 10;

#[derive(Default, Deref, Resource)]
struct AgentCancelToken(Arc<CancellationToken>);

#[derive(Deref, DerefMut, Resource)]
struct AgentRuntime(Arc<SingleThreadedRuntime>);

#[derive(Deref, DerefMut, Resource)]
struct CodingTopic(Topic<Task>);

fn setup(
    mut commands: Commands<'_, '_>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
    mut tui: NonSendMut<TuiMain<'_>>,
    app_cancel: Res<'_, AppCancelToken>,
    agent_cancel: Res<'_, AgentCancelToken>,
) -> bevy::ecs::error::Result<()> {
    let api_key: String = dotenvy::var("OPENAI_API_KEY")
        .map_err::<BevyError, fn(dotenvy::Error) -> BevyError>(
            |_: dotenvy::Error| -> BevyError { BevyError::from("OPENAI_API_KEY must be set") },
        )?;
    let base_url: String = dotenvy::var::<&str>("BASE_URL").unwrap_or_default();
    let model: String = dotenvy::var::<&str>("MODEL").unwrap_or(String::from("gpt-4o"));

    let llm: Arc<dyn LLMProvider> = {
        let mut builder: LLMBuilder<OpenAI> = LLMBuilder::<OpenAI>::new()
            .api_key(&api_key)
            .model(&model)
            .max_tokens(MAX_TOKENS)
            .temperature(0.1);

        if !base_url.is_empty() {
            builder = builder.base_url(&base_url);
        }

        builder
            .build()
            .map_err::<BevyError, fn(LLMError) -> BevyError>(|_: LLMError| -> BevyError {
                BevyError::from("Failed to build LLM")
            })?
    };

    () = tui.output.push(Line::<'_>::from(
        "🚀 Starting Interactive Coding Agent Session",
    ));

    let memory: Box<SlidingWindowMemory> =
        Box::<SlidingWindowMemory>::new(SlidingWindowMemory::new(300));

    let runtime: Arc<SingleThreadedRuntime> = SingleThreadedRuntime::new(None);
    () = commands.insert_resource::<AgentRuntime>(AgentRuntime(runtime.clone()));

    let coding_topic: Topic<Task> = Topic::<Task>::new(CODING_TASK_TOPIC);
    () = commands.insert_resource::<CodingTopic>(CodingTopic(coding_topic.clone()));

    let coding_agent: ReActAgent<CodingAgent> = ReActAgent::<CodingAgent>::new(CodingAgent {});

    let app_cancel: Arc<CancellationToken> = app_cancel.clone();
    let agent_cancel: Arc<CancellationToken> = agent_cancel.clone();
    let _: JoinHandle<Result<(), autoagents::core_error::Error>> = tokio_runtime
        .spawn_background_task::<_, Result<(), autoagents::core_error::Error>, _>(
            |mut ctx: TaskContext| async move {
                let _: ActorAgentHandle<ReActAgent<CodingAgent>> = AgentBuilder::new(coding_agent)
                    .llm(llm)
                    .runtime(runtime.clone())
                    .subscribe(coding_topic.clone())
                    .memory(memory)
                    .build()
                    .await?;

                let mut environment: Environment = Environment::new(None);
                () = environment.register_runtime(runtime.clone()).await?;

                let _handle: JoinHandle<Result<(), RuntimeError>> = environment.run();

                let mut receiver: BoxEventStream<Event> =
                    environment.take_event_receiver(None).await?;

                loop {
                    tokio::select! {
                        Some(event) = receiver.next() => {
                            () = ctx.run_on_main_thread::<_, ()>(|ctx: MainThreadContext<'_>| {
                                let _: Option<MessageId<ProtocolEvent>> = ctx.world
                                    .write_message::<ProtocolEvent>(ProtocolEvent(event));
                            }).await;
                        }
                        _ = app_cancel.cancelled() => break,
                        _ = agent_cancel.cancelled() => break,
                        else => unreachable!(),
                    }
                }

                () = environment.shutdown().await;

                Ok::<(), autoagents::core_error::Error>(())
            },
        );

    Ok(())
}

#[derive(Deref, DerefMut, Message)]
struct ProtocolEvent(Event);

fn handle_protocol_events(
    mut messages: MessageReader<'_, '_, ProtocolEvent>,
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    mut commands: Commands<'_, '_>,
    mut processing: Option<Res<'_, ProcessingTask>>,
    mut agent_request_writer: MessageWriter<'_, CodingAgentRequest>,
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
                match serde_json::from_str::<ReActAgentOutput>(result) {
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
                        //Do Nothing
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
                    () = commands.remove_resource::<ProcessingTask>();
                } else if turn_number + 1 >= MAX_TURNS
                    && let Some(processing) = processing.take()
                {
                    let ProcessingTask(task) = processing.into_inner();
                    let _: MessageId<CodingAgentRequest> =
                        agent_request_writer.write(CodingAgentRequest(task.prompt.clone()));
                    () = commands.remove_resource::<ProcessingTask>()
                }
            }
            _ => {
                // Handle other events silently or with debug output
            }
        }
    }

    Ok(())
}

#[derive(Deref, DerefMut, Message)]
pub struct CodingAgentRequest(pub String);

#[derive(Deref, Resource)]
struct ProcessingTask(Task);

fn spawn_agent_task(
    runtime: ResMut<'_, TokioTasksRuntime>,
    mut messages: MessageReader<'_, '_, CodingAgentRequest>,
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    mut commands: Commands<'_, '_>,
    agent_runtime: Res<'_, AgentRuntime>,
    coding_topic: Res<'_, CodingTopic>,
) {
    for CodingAgentRequest(input) in messages.read() {
        let output: &mut Vec<Line<'_>> = tui.output.as_mut();

        let span: Span<'_> = Span::<'_>::raw("");
        let line: Line<'_> = Line::<'_>::from(span);
        () = output.push(line);

        let span: Span<'_> = Span::<'_>::raw("🔄 Processing your request...\n");
        let line: Line<'_> = Line::<'_>::from(span);
        () = output.push(line);

        let task: Task = Task::new(input);
        () = commands.insert_resource::<ProcessingTask>(ProcessingTask(task.clone()));

        let agent_runtime: Arc<SingleThreadedRuntime> = agent_runtime.clone();
        let coding_topic: Topic<Task> = coding_topic.clone();

        let _: JoinHandle<()> =
            runtime.spawn_background_task::<_, (), _>(|_ctx: TaskContext| async move {
                let _: Result<(), RuntimeError> =
                    agent_runtime.publish::<Task>(&coding_topic, task).await;
            });
    }
}

fn shutdown_agent_environment_on_exit(
    mut messages: MessageReader<'_, '_, AppExit>,
    mut cancel: Option<Res<'_, AgentCancelToken>>,
) {
    for _message in messages.read() {
        if let Some(cancel) = cancel.take()
            && !cancel.is_cancelled()
        {
            () = cancel.cancel();
        }
    }
}

pub fn agents_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<AgentCancelToken>()
        .add_message::<ProtocolEvent>()
        .add_message::<CodingAgentRequest>()
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // Commands<'_, '_>
                _, // ResMut<'_, TokioTasksRuntime>
                _, // NonSendMut<TuiMain<'_>>
                _, // Res<'_, AppCancelToken>
                _, // Res<'_, AgentCancelToken>
            ) -> bevy::ecs::error::Result<()>,
        )>(Startup, setup)
        .add_systems::<(
            ScheduleConfigTupleMarker,
            (
                IsFunctionSystem,
                fn(
                    _, // MessageReader<'_, '_, ProtocolEvent>
                    _, // NonSendMut<'_, TuiMain<'_>>
                    _, // Commands<'_, '_>
                    _, // Option<Res<'_, ProcessingTask>>
                    _, // MessageWriter<'_, CodingAgentRequest>
                ) -> bevy::ecs::error::Result,
            ),
            (),
            (
                IsFunctionSystem,
                fn(
                    _, // MessageReader<'_, '_, AppExit>
                    _, // Option<Res<'_, AgentCancelToken>>
                ) -> (),
            ),
        )>(
            Update,
            (
                handle_protocol_events,
                spawn_agent_task.run_if::<_>(not(resource_exists::<ProcessingTask>)),
                shutdown_agent_environment_on_exit,
            ),
        );
}
