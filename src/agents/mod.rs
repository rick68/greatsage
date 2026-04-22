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
    std::{env, path::PathBuf, sync::Arc},
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

#[derive(Resource)]
pub struct PermissionConfig {
    pub allowed_dir: PathBuf,
}

#[allow(dead_code)]
impl PermissionConfig {
    pub fn is_path_allowed(&self, path: &std::path::Path) -> bool {
        // Canonicalize both paths to handle relative components
        if let Ok(canonical_allowed) = self.allowed_dir.canonicalize()
            && let Ok(canonical_target) = path.canonicalize()
        {
            canonical_target.starts_with::<&PathBuf>(&canonical_allowed)
        } else {
            false
        }
    }
}

impl Default for PermissionConfig {
    fn default() -> Self {
        let cwd: PathBuf = env::current_dir().unwrap_or_else::<fn(std::io::Error) -> PathBuf>(
            |_: std::io::Error| -> PathBuf { PathBuf::from(".") },
        );

        Self { allowed_dir: cwd }
    }
}

fn setup(
    app_cancel: Res<'_, AppCancelToken>,
    agents_cancel: Res<'_, AgentsCancelToken>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
) {
    let app_cancel: Arc<CancellationToken> = app_cancel.clone();
    let agents_cancel: Arc<CancellationToken> = agents_cancel.clone();

    let _: JoinHandle<()> =
        tokio_runtime.spawn_background_task::<_, (), _>(|_ctx: TaskContext| async move {
            tokio::select! {
                _ = app_cancel.cancelled() => (),
                _ = agents_cancel.cancelled() => (),
                else => unreachable!(),
            }
        });
}

pub fn agents_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<LlmConfig>()
        .init_resource::<AgentsCancelToken>()
        .init_resource::<PermissionConfig>()
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
