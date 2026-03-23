use {
    crate::{
        agents::{
            AgentsCancelToken, GlobalAgentEnvironoment, GlobalAgentRuntime, Llm, MAX_TURNS,
            SharedSlidingWindowMemory,
            tools::{AnalyzeCodeTool, GrepTool},
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
    autoagents_toolkit::tools::{
        document_parsing::DocumentParser,
        filesystem::{
            CopyFile, CreateDir, DeleteFile, ListDir, MoveFile, ReadFile, SearchFile, WriteFile,
        },
    },
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

const CODING_TASK_TOPIC: &str = "coding_task";

#[derive(Deref, DerefMut, Message)]
pub struct CodingAgentRequest(pub String);

#[derive(Deref, DerefMut, Message)]
struct CodingAgentProtocolEvent(Event);

#[agent(
    name = "coding_agent",
    description = "You are a coding agent operating within the AutoAgents framework using the ReAct (Reasoning + Acting) execution pattern. Your primary role is to help users with software engineering tasks through systematic reasoning and tool usage.

## Core Capabilities
You can:
- Search for files using glob patterns (FileSearchTool)
- Search file contents with regex patterns (GrepTool)
- Read file contents (ReadFileTool)
- Write and create files (WriteFileTool)
- Delete files (DeleteFileTool)
- List directory contents (ListDirectoryTool)
- Analyze code structure and complexity (AnalyzeCodeTool)

## ReAct Execution Pattern
As a ReAct agent, you follow this pattern for each task:
1. **Thought**: Analyze what needs to be done and plan your approach
2. **Action**: Use appropriate tools to gather information or make changes
3. **Observation**: Process the results from your tools
4. **Repeat**: Continue the thought-action-observation cycle until the task is complete

## Working Principles
- **Be Precise**: Always use exact file paths. When given a working directory, use it as the base for all operations
- **Verify Before Acting**: Check if files/directories exist before attempting operations
- **Incremental Progress**: Break complex tasks into smaller, manageable steps
- **Clear Communication**: Explain your reasoning and actions, but be concise
- **Safety First**: Never delete or overwrite files without clear intent
- **Follow Conventions**: Respect existing code style and project structure

## Task Execution Guidelines
- Start by understanding the codebase structure using ListDirectoryTool or FileSearchTool
- Use GrepTool to find patterns across multiple files efficiently
- Read files to understand context before making modifications
- When writing code, follow the existing style and conventions
- Always provide clear feedback about what was accomplished

## Important Constraints
- All file paths should be relative to the provided base directory
- You cannot execute shell commands or run code directly
- Focus on file manipulation and code analysis tasks
- Be explicit about limitations when you cannot complete a request

Remember: You are a systematic problem solver. Think through each step, use your tools effectively, and provide clear, actionable results.",
    tools = [
        CreateDir::new(),
        ListDir::new(),
        GrepTool,
        CopyFile::new(),
        DeleteFile::new(),
        MoveFile::new(),
        ReadFile::new(),
        SearchFile::new(100),
        WriteFile::new(),
        AnalyzeCodeTool,
        DocumentParser,
    ],
)]
#[derive(AgentHooks, Clone)]
struct CodingAgent {}

#[derive(Deref, DerefMut, Resource)]
struct CodingTopic(Topic<Task>);

fn coding_topic_setup(mut commands: Commands<'_, '_>) {
    let coding_topic: Topic<Task> = Topic::<Task>::new(CODING_TASK_TOPIC);
    () = commands.insert_resource::<CodingTopic>(CodingTopic(coding_topic));
}

fn setup(
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    llm: Res<'_, Llm>,
    agent_runtime: Res<'_, GlobalAgentRuntime>,
    coding_topic: Res<'_, CodingTopic>,
    shared_memory: Res<'_, SharedSlidingWindowMemory>,
    app_cancel: Res<'_, AppCancelToken>,
    agents_cancel: Res<'_, AgentsCancelToken>,
    global_agent_environment: Res<'_, GlobalAgentEnvironoment>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
) -> bevy::ecs::error::Result<()> {
    () = tui.output.push(Line::<'_>::from(
        "🚀 Starting Interactive Coding Agent Session",
    ));

    let coding_agent: ReActAgent<CodingAgent> = ReActAgent::<CodingAgent>::new(CodingAgent {});
    let llm: Arc<dyn LLMProvider> = llm.clone();
    let agent_runtime: Arc<SingleThreadedRuntime> = agent_runtime.clone();
    let coding_topic: Topic<Task> = coding_topic.clone();
    let shared_memory: Box<SharedSlidingWindowMemory> = Box::new(shared_memory.clone());
    let app_cancel: Arc<CancellationToken> = app_cancel.clone();
    let agents_cancel: Arc<CancellationToken> = agents_cancel.clone();
    let global_agent_environment: Arc<Mutex<Environment>> = global_agent_environment.clone();

    let _: JoinHandle<_> =
        tokio_runtime.spawn_background_task::<_, _, _>(|mut ctx: TaskContext| async move {
            let _: ActorAgentHandle<ReActAgent<CodingAgent>> = AgentBuilder::new(coding_agent)
                .llm(llm)
                .runtime(agent_runtime.clone())
                .subscribe(coding_topic.clone())
                .memory(shared_memory)
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
                            let _: Option<MessageId<CodingAgentProtocolEvent >> = ctx.world
                                .write_message::<CodingAgentProtocolEvent>(CodingAgentProtocolEvent(event));
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
struct ProcessingCodingTask(Task);

fn handle_protocol_events(
    mut messages: MessageReader<'_, '_, CodingAgentProtocolEvent>,
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    mut commands: Commands<'_, '_>,
    mut processing: Option<Res<'_, ProcessingCodingTask>>,
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
                    () = commands.remove_resource::<ProcessingCodingTask>();
                } else if turn_number + 1 >= MAX_TURNS
                    && let Some(processing) = processing.take()
                {
                    () = commands.remove_resource::<ProcessingCodingTask>();
                    let ProcessingCodingTask(task) = processing.into_inner();
                    let _: MessageId<CodingAgentRequest> =
                        agent_request_writer.write(CodingAgentRequest(task.prompt.clone()));
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
    mut messages: MessageReader<'_, '_, CodingAgentRequest>,
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    mut commands: Commands<'_, '_>,
    agent_runtime: Res<'_, GlobalAgentRuntime>,
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
        () = commands.insert_resource::<ProcessingCodingTask>(ProcessingCodingTask(task.clone()));

        let agent_runtime: Arc<SingleThreadedRuntime> = agent_runtime.clone();
        let coding_topic: Topic<Task> = coding_topic.clone();

        let _: JoinHandle<()> =
            runtime.spawn_background_task::<_, (), _>(|_ctx: TaskContext| async move {
                let _: Result<(), RuntimeError> =
                    agent_runtime.publish::<Task>(&coding_topic, task).await;
            });
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
    let _: &mut App = app
        .add_message::<CodingAgentRequest>()
        .add_message::<CodingAgentProtocolEvent>()
        .add_systems::<()>(PostStartup, (coding_topic_setup, setup).chain())
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
                    fn(Option<Res<'_, ProcessingCodingTask>>) -> bool,
                >(
                    resource_exists::<ProcessingCodingTask>
                )),
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
