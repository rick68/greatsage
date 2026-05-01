mod coding;
pub use coding::{CodingAgentPromptChannel, CodingAgentTask, coding_agent_plugin};

use {
    crate::tokio::AppCancelToken,
    bevy::{
        app::{App, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            resource::Resource,
        },
        prelude::Deref,
    },
    bevy_tokio_tasks::TokioTasksRuntime,
    std::sync::Arc,
    tokio_util::sync::CancellationToken,
};

#[derive(Resource)]
pub struct AgentConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            base_url: dotenvy::var("BASE_URL").unwrap_or_default(),
            model: dotenvy::var("MODEL").unwrap_or_default(),
            api_key: dotenvy::var("API_KEY").unwrap_or_default(),
        }
    }
}

#[derive(Default, Deref, Resource)]
struct AgentsCancelToken(Arc<CancellationToken>);

fn setup(
    app_cancel: Res<AppCancelToken>,
    agents_cancel: Res<AgentsCancelToken>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
) {
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

pub fn agents_plugin(app: &mut App) {
    app.init_resource::<AgentConfig>()
        .init_resource::<AgentsCancelToken>()
        .add_plugins(coding_agent_plugin)
        .add_systems(Startup, setup);
}
