mod coding;
pub use coding::CodingAgentRequest;

mod tools;

use {
    self::coding::coding_agent_plugin,
    crate::tokio::AppCancelToken,
    autoagents::{
        core::{environment::Environment, runtime::RuntimeError, runtime::SingleThreadedRuntime},
        llm::{LLMProvider, backends::openai::OpenAI, builder::LLMBuilder},
        llm_error::LLMError,
    },
    bevy::{
        app::{App, PreStartup, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            error::BevyError,
            resource::Resource,
            schedule::IntoScheduleConfigs,
            system::{Commands, IsFunctionSystem},
        },
        prelude::{Deref, DerefMut},
    },
    bevy_tokio_tasks::{TaskContext, TokioTasksRuntime},
    std::sync::Arc,
    tokio::{sync::Mutex, task::JoinHandle},
    tokio_util::sync::CancellationToken,
};

const MAX_TOKENS: u32 = 131_072;
const MAX_TURNS: usize = 10;

#[derive(Deref, DerefMut, Resource)]
struct Llm(Arc<dyn LLMProvider>);

#[derive(Deref, DerefMut, Resource)]
struct GlobalAgentRuntime(Arc<SingleThreadedRuntime>);

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

fn runtime_setup(mut commands: Commands<'_, '_>) {
    let runtime: Arc<SingleThreadedRuntime> = SingleThreadedRuntime::new(None);
    () = commands.insert_resource::<GlobalAgentRuntime>(GlobalAgentRuntime(runtime));
}

#[derive(Deref, DerefMut, Resource)]
pub struct GlobalAgentEnvironoment(Arc<Mutex<Environment>>);

impl Default for GlobalAgentEnvironoment {
    fn default() -> Self {
        let environment: Environment = Environment::new(None);
        GlobalAgentEnvironoment(Arc::new(Mutex::new(environment)))
    }
}

fn global_agent_environment_setup(
    global_agent_environoment: Res<'_, GlobalAgentEnvironoment>,
    global_agent_runtime: Res<'_, GlobalAgentRuntime>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
    app_cancel: Res<'_, AppCancelToken>,
    agents_cancel: Res<'_, AgentsCancelToken>,
) {
    let global_agent_environoment: Arc<Mutex<Environment>> = global_agent_environoment.clone();
    let global_agent_runtime: Arc<SingleThreadedRuntime> = global_agent_runtime.clone();
    let app_cancel: Arc<CancellationToken> = app_cancel.clone();
    let agents_cancel: Arc<CancellationToken> = agents_cancel.clone();
    let _: JoinHandle<Result<(), autoagents::core_error::Error>> = tokio_runtime
        .spawn_background_task::<_, Result<(), autoagents::core_error::Error>, _>(
            |_ctx: TaskContext| async move {
                () = global_agent_environoment
                    .lock()
                    .await
                    .register_runtime(global_agent_runtime)
                    .await?;

                let _: JoinHandle<Result<(), RuntimeError>> =
                    global_agent_environoment.lock().await.run();

                let _: JoinHandle<()> = tokio::spawn::<_>(async move {
                    loop {
                        tokio::select! {
                            _ = app_cancel.cancelled() => {
                                () = global_agent_environoment.lock().await.shutdown().await;
                                break;
                            },
                            _ = agents_cancel.cancelled() => {
                                () = global_agent_environoment.lock().await.shutdown().await;
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

pub fn agents_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<AgentsCancelToken>()
        .init_resource::<GlobalAgentEnvironoment>()
        .add_plugins::<_>(coding_agent_plugin)
        .add_systems::<_>(PreStartup, (llm_setup, runtime_setup).chain())
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // Res<'_, GlobalAgentEnvironoment>
                _, // Res<'_, GlobalAgentRuntime>
                _, // ResMut<'_, TokioTasksRuntime>
                _, // Res<'_, AppCancelToken>
                _, // Res<'_, AgentsCancelToken>
            ) -> (),
        )>(Startup, global_agent_environment_setup);
}
