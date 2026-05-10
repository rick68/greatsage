mod coding;
pub use coding::{
    CodingAgent, CodingAgentPromptChannel, CodingAgentTask, SYSTEM_PROMPT, coding_agent_plugin,
};

use {
    crate::{
        config::{Config, McpConfig},
        tokio::AppCancelToken,
    },
    bevy::{
        app::{App, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            resource::Resource,
            system::Commands,
        },
        prelude::Deref,
    },
    bevy_tokio_tasks::TokioTasksRuntime,
    std::sync::Arc,
    tokio_util::sync::CancellationToken,
    yoagent::SkillSet,
};

#[derive(Clone, Debug, Resource)]
pub struct AgentConfig {
    pub model: String,
    pub base_url: String,
    pub skills: SkillSet,
    pub system_prompt: String,
    pub api_key: String,
    pub mcp: Vec<McpConfig>,
}

impl From<&Config> for AgentConfig {
    fn from(config: &Config) -> Self {
        let model = config.get_model().unwrap_or_default();
        let base_url = config.get_base_url().unwrap_or_default();
        let skills = SkillSet::load(config.get_skills().as_slice()).expect("Failed to load skills");
        let system_prompt = config.get_system_prompt();
        let api_key = config.get_api_key().unwrap_or_default();
        let mcp = config.get_mcp();

        AgentConfig {
            model,
            base_url,
            skills,
            system_prompt,
            api_key,
            mcp,
        }
    }
}

#[derive(Default, Deref, Resource)]
pub struct AgentsCancelToken(Arc<CancellationToken>);

fn setup(
    config: Res<Config>,
    mut commands: Commands,
    app_cancel: Res<AppCancelToken>,
    agents_cancel: Res<AgentsCancelToken>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
) {
    let agent_config = AgentConfig::from(config.into_inner());
    commands.insert_resource(agent_config);

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
    app.init_resource::<AgentsCancelToken>()
        .add_plugins(coding_agent_plugin)
        .add_systems(Startup, setup);
}
