use {
    crate::{
        agents::{
            AgentsCancelToken, Llm, coding::CodingAgentRequest, companion::CompanionAgentRequest,
        },
        tui::TuiMain,
    },
    autoagents::{
        core::agent::{
            AgentBuilder, DirectAgent, DirectAgentHandle, prebuilt::executor::BasicAgent,
            task::Task,
        },
        llm::LLMProvider,
    },
    autoagents_derive::{AgentHooks, agent},
    bevy::{
        app::{App, AppExit, Startup, Update},
        ecs::{
            change_detection::{Mut, NonSendMut, Res, ResMut},
            message::{Message, MessageId, MessageReader},
            resource::Resource,
            schedule::{
                IntoScheduleConfigs, SystemCondition,
                common_conditions::{not, resource_exists},
            },
            system::{Commands, IsFunctionSystem},
        },
        prelude::Deref,
    },
    bevy_tokio_tasks::{MainThreadContext, TaskContext, TokioTasksRuntime},
    ratatui::{style::Stylize, text::Line},
    std::sync::Arc,
    tokio::{sync::Mutex, task::JoinHandle},
};

#[agent(
    name = "routing_agent",
    description = r#"You are the Routing Agent, the intelligent central dispatcher in the multi-agent system.

Your only responsibility is to analyze the user's message and route it to the correct specialist.
Never explain, never add extra text.

## Available specialists:
- "coder": Handles ANY programming, code writing, debugging, architecture, project analysis, codebase review, refactoring, algorithms, or technical implementation tasks.
  Any codebase-related query MUST go here.
- "companion": Handles general chat, casual conversation, knowledge questions, brainstorming, or non-technical topics.

ONLY output one word: 'coder', or 'companion'"#
)]
#[derive(AgentHooks, Clone)]
struct RoutingAgent {}

#[derive(Deref, Resource)]
struct Router(Arc<Mutex<DirectAgentHandle<BasicAgent<RoutingAgent>>>>);

fn setup(
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    llm: Res<'_, Llm>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
) -> bevy::ecs::error::Result<()> {
    () = tui.output.push(Line::<'_>::from(
        "🚀 Starting Interactive Routing Agent Session",
    ));

    let agent: BasicAgent<RoutingAgent> = BasicAgent::<RoutingAgent>::new(RoutingAgent {});
    let llm: Arc<dyn LLMProvider> = llm.clone();

    let _: JoinHandle<Result<(), autoagents::core_error::Error>> = tokio_runtime
        .spawn_background_task::<_, Result<(), autoagents::core_error::Error>, _>(
            |mut ctx: TaskContext| async move {
                let agent_handle: DirectAgentHandle<BasicAgent<RoutingAgent>> =
                    AgentBuilder::<BasicAgent<RoutingAgent>, DirectAgent>::new(agent)
                        .llm(llm.clone())
                        .build()
                        .await?;

                () = ctx
                    .run_on_main_thread::<_, ()>(|ctx: MainThreadContext<'_>| {
                        ctx.world
                            .insert_resource::<Router>(Router(Arc::new(Mutex::new(agent_handle))));
                    })
                    .await;

                Ok::<(), autoagents::core_error::Error>(())
            },
        );

    Ok::<(), bevy::ecs::error::BevyError>(())
}

#[derive(Deref, Message)]
pub struct RoutingAgentRequest(pub String);

#[derive(Deref, Resource)]
struct ProcessingRoutingTask(Task);

fn handle_routing(
    mut messages: MessageReader<'_, '_, RoutingAgentRequest>,
    processing: Res<'_, Router>,
    runtime: ResMut<'_, TokioTasksRuntime>,
    mut commands: Commands<'_, '_>,
) {
    for RoutingAgentRequest(input) in messages.read() {
        let task: Task = Task::new(input.clone());
        () = commands.insert_resource::<ProcessingRoutingTask>(ProcessingRoutingTask(task.clone()));
        let orchestrator: Arc<Mutex<DirectAgentHandle<BasicAgent<RoutingAgent>>>> =
            processing.clone();
        let _: JoinHandle<
            Result<
                (),
                autoagents::core::agent::error::RunnableAgentError>,
        > = runtime
            .spawn_background_task::<
                _,
                Result<(), autoagents::core::agent::error::RunnableAgentError>,
                _,
            >(move |mut ctx: TaskContext| async move {
                let input: String = task.prompt.clone();
                let decision: String = orchestrator.lock().await.agent.run(task.clone()).await?;

                () = ctx.run_on_main_thread(move |ctx:  MainThreadContext<'_>| {
                    let mut tui: Mut<'_, TuiMain<'_>> = ctx.world.non_send_resource_mut::<TuiMain<'_>>();

                    () = tui.output.push(Line::<'_>::from(
                        format!("Routing to [{decision}]").yellow().bold(),
                    ));
                    () = tui.scroll_to_bottom();

                    match decision.as_str() {
                        "coder" => {
                            let _: Option<MessageId<CodingAgentRequest> >=
                                ctx.world.write_message::<CodingAgentRequest>(CodingAgentRequest(input));
                        }
                        "companion" => {
                            let _: Option<MessageId<CompanionAgentRequest> >=
                                ctx.world.write_message::<CompanionAgentRequest>(CompanionAgentRequest(input));
                        }
                        _ => (),
                    }
                    let _: Option<ProcessingRoutingTask> = ctx.world.remove_resource::<ProcessingRoutingTask>();
                }).await;

                Ok::<(), autoagents::core::agent::error::RunnableAgentError>(())
            });
    }
}

fn shutdown_routing_agent(
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

pub fn routing_agent_plugin(app: &mut App) {
    let _: &mut App = app
        .add_message::<RoutingAgentRequest>()
        .add_systems::<_>(Startup, setup)
        .add_systems(
            Update,
            (
                handle_routing.run_if::<()>(
                    not::<
                        (
                            IsFunctionSystem,
                            fn(
                                Option<
                                    _, // Res<'_, ProcessingRoutingTask>
                                >,
                            ) -> bool,
                        ),
                        bool,
                        fn(Option<Res<'_, ProcessingRoutingTask>>) -> bool,
                    >(resource_exists::<ProcessingRoutingTask>)
                    .and::<(
                        IsFunctionSystem,
                        fn(
                            Option<
                                _, // Res<'_, Router>
                            >,
                        ) -> bool,
                    ), fn(Option<Res<'_, Router>>) -> bool>(
                        resource_exists::<Router>
                    ),
                ),
                shutdown_routing_agent,
            ),
        );
}
