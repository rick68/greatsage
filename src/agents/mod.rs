mod coding;

use bevy::ecs::system::IsFunctionSystem;
pub use coding::{CodingAgentPromptChannel, CodingAgentTask};

use {
    self::coding::coding_agent_plugin,
    crate::tokio::AppCancelToken,
    bevy::{
        app::{App, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            resource::Resource,
        },
        prelude::Deref,
    },
    bevy_tokio_tasks::{TaskContext, TokioTasksRuntime},
    std::sync::Arc,
    tokio::task::JoinHandle,
    tokio_util::sync::CancellationToken,
};

#[derive(Resource)]
pub struct LlmConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            base_url: dotenvy::var::<&str>("BASE_URL").unwrap_or_default(),
            model: dotenvy::var::<&str>("MODEL").unwrap_or_default(),
            api_key: dotenvy::var::<&str>("API_KEY").unwrap_or_default(),
        }
    }
}

#[derive(Default, Deref, Resource)]
struct AgentsCancelToken(Arc<CancellationToken>);

fn setup(
    app_cancel: Res<'_, AppCancelToken>,
    agents_cancel: Res<'_, AgentsCancelToken>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
) {
    let app_cancel: Arc<CancellationToken> = app_cancel.clone();
    let agents_cancel: Arc<CancellationToken> = agents_cancel.clone();

    let _: JoinHandle<()> =
        tokio_runtime.spawn_background_task::<_, (), _>(|_ctx: TaskContext| async move {
            loop {
                tokio::select! {
                    _ = app_cancel.cancelled() => break,
                    _ = agents_cancel.cancelled() => break,
                    else => unreachable!(),
                }
            }
        });
}

pub fn agents_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<LlmConfig>()
        .init_resource::<AgentsCancelToken>()
        .add_plugins::<_>(coding_agent_plugin)
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // Res<'_, AppCancelToken>
                _, // Res<'_, AgentsCancelToken>
                _, // ResMut<'_, TokioTasksRuntime>
            ) -> (),
        )>(Startup, setup);
}
