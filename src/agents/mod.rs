mod coding;
mod companion;

mod routing;
pub use routing::RoutingAgentRequest;

mod shared_memory;
pub use shared_memory::SharedSlidingWindowMemory;

mod tools;

use {
    self::{
        coding::coding_agent_plugin, companion::companion_agent_plugin,
        routing::routing_agent_plugin,
    },
    crate::tokio::AppCancelToken,
    autoagents::{
        core::{
            environment::Environment,
            runtime::{RuntimeError, SingleThreadedRuntime},
            utils::BoxEventStream,
        },
        llm::{LLMProvider, backends::openai::OpenAI, builder::LLMBuilder},
        llm_error::LLMError,
        protocol::Event,
    },
    bevy::{
        app::{App, PreStartup, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            error::BevyError,
            message::{Message, MessageId},
            resource::Resource,
            schedule::IntoScheduleConfigs,
            system::{Commands, IsFunctionSystem},
        },
        prelude::{Deref, DerefMut},
        tasks::futures_lite::StreamExt,
    },
    bevy_tokio_tasks::{MainThreadContext, TaskContext, TokioTasksRuntime},
    std::sync::Arc,
    tokio::{sync::Mutex, task::JoinHandle},
    tokio_util::sync::CancellationToken,
};

const MAX_TOKENS: u32 = 131_072;
const MAX_TURNS: usize = 10;

#[derive(Deref, DerefMut, Resource)]
struct Llm(Arc<dyn LLMProvider>);

#[derive(Default, Deref, Resource)]
struct AgentsCancelToken(Arc<CancellationToken>);

fn llm_setup(mut commands: Commands<'_, '_>) -> bevy::ecs::error::Result<()> {
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

    () = commands.insert_resource::<Llm>(Llm(llm));

    Ok(())
}

#[derive(Deref, DerefMut, Resource)]
pub struct GlobalAgentEnvironment(Arc<Mutex<Environment>>);

impl Default for GlobalAgentEnvironment {
    fn default() -> Self {
        let environment: Environment = Environment::new(None);
        GlobalAgentEnvironment(Arc::new(Mutex::new(environment)))
    }
}

#[derive(Deref, DerefMut, Resource)]
struct GlobalAgentRuntime(Arc<SingleThreadedRuntime>);

fn global_agent_environment_setup(
    global_agent_environment: Res<'_, GlobalAgentEnvironment>,
    global_agent_runtime: Res<'_, GlobalAgentRuntime>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
    app_cancel: Res<'_, AppCancelToken>,
    agents_cancel: Res<'_, AgentsCancelToken>,
) {
    let global_agent_environment: Arc<Mutex<Environment>> = global_agent_environment.clone();
    let global_agent_runtime: Arc<SingleThreadedRuntime> = global_agent_runtime.clone();
    let app_cancel: Arc<CancellationToken> = app_cancel.clone();
    let agents_cancel: Arc<CancellationToken> = agents_cancel.clone();

    let _: JoinHandle<Result<(), autoagents::core_error::Error>> = tokio_runtime
        .spawn_background_task::<_, Result<(), autoagents::core_error::Error>, _>(
            |_ctx: TaskContext| async move {
                () = global_agent_environment
                    .lock()
                    .await
                    .register_runtime(global_agent_runtime)
                    .await?;

                let _: JoinHandle<Result<(), RuntimeError>> =
                    global_agent_environment.lock().await.run();

                let _: JoinHandle<()> = tokio::spawn::<_>(async move {
                    loop {
                        tokio::select! {
                            _ = app_cancel.cancelled() => {
                                () = global_agent_environment.lock().await.shutdown().await;
                                break;
                            },
                            _ = agents_cancel.cancelled() => {
                                () = global_agent_environment.lock().await.shutdown().await;
                                break;
                            },
                            else => unreachable!(),
                        }
                    }
                });

                Ok::<(), autoagents::core_error::Error>(())
            },
        );
}

#[derive(Deref, DerefMut, Message)]
pub struct ProtocolEvent(Event);

fn protocol_event_forward_setup(
    global_agent_environment: Res<'_, GlobalAgentEnvironment>,
    app_cancel: Res<'_, AppCancelToken>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
) {
    let global_agent_environment: Arc<Mutex<Environment>> = global_agent_environment.clone();
    let app_cancel: Arc<CancellationToken> = app_cancel.clone();

    let _: JoinHandle<Result<(), autoagents::core_error::Error>> = tokio_runtime
        .spawn_background_task::<_, Result<(), autoagents::core_error::Error>, _>(
            |mut ctx: TaskContext| async move {
                let mut receiver: BoxEventStream<Event> = global_agent_environment
                    .lock()
                    .await
                    .take_event_receiver(None)
                    .await?;

                loop {
                    tokio::select! {
                        Some(event) = receiver.next() => {
                            () = ctx.run_on_main_thread::<_, ()>(move |ctx:  MainThreadContext<'_>| {
                                let _: Option<MessageId<ProtocolEvent>> =
                                    ctx.world.write_message::<ProtocolEvent>(ProtocolEvent(event));
                            }).await;
                        }
                        _ = app_cancel.cancelled() => break,
                        else => unreachable!(),
                    }
                }

                Ok::<(), autoagents::core_error::Error>(())
            },
        );
}

pub fn agents_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<AgentsCancelToken>()
        .init_resource::<GlobalAgentEnvironment>()
        .init_resource::<SharedSlidingWindowMemory>()
        .insert_resource::<GlobalAgentRuntime>(GlobalAgentRuntime(SingleThreadedRuntime::new(None)))
        .add_message::<ProtocolEvent>()
        .add_plugins::<(_, _, _, _)>((
            coding_agent_plugin,
            companion_agent_plugin,
            routing_agent_plugin,
        ))
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // Commands<'_, '_>
            ) -> bevy::ecs::error::Result<()>,
        )>(PreStartup, llm_setup)
        .add_systems::<()>(
            Startup,
            (global_agent_environment_setup, protocol_event_forward_setup).chain(),
        );
}
