mod coding;
pub(crate) use coding::{
    CodingAgent, CodingAgentEvent, CodingAgentPromptChannel, CodingAgentTask, SYSTEM_PROMPT,
    coding_agent_plugin, install_coding_agent, prepare_coding_agent_preserving_messages,
};

use {
    crate::{
        cli::Cli,
        config::{Config, McpConfig},
        project_context::assemble_system_prompt,
        providers::Provider,
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
    std::{env, path::PathBuf, sync::Arc},
    tokio_util::sync::CancellationToken,
    yoagent::SkillSet,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AgentConfigOptions {
    pub bare: bool,
    pub explicit_skills: bool,
    pub explicit_mcp: bool,
}

impl AgentConfigOptions {
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            bare: cli.bare,
            explicit_skills: cli.skills.is_some(),
            explicit_mcp: cli.mcp.is_some(),
        }
    }
}

#[derive(Clone, Debug, Resource)]
pub struct AgentConfig {
    pub model: String,
    pub provider: Provider,
    pub base_url: String,
    pub skills: SkillSet,
    pub system_prompt: String,
    pub api_key: String,
    pub mcp: Vec<McpConfig>,
}

impl AgentConfig {
    /// Build agent config. Pass [`AgentConfigOptions::from_cli`] at startup so `--bare`
    /// can skip auto-loaded project context, skills, and MCP (Claude Code bare parity).
    pub fn from_config(config: &Config, opts: AgentConfigOptions) -> Self {
        let bare = opts.bare;
        let provider = config.get_provider();
        let model = config
            .get_model()
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| {
                provider
                    .map(|provider| provider.default_model().to_owned())
                    .unwrap_or_else(|| "claude-fable-5".to_owned())
            });
        let base_url = config.get_base_url().unwrap_or_default();
        let skill_paths = if bare && !opts.explicit_skills {
            Vec::new()
        } else {
            config.get_skills()
        };
        let skills = SkillSet::load(skill_paths.as_slice()).expect("Failed to load skills");
        let base_prompt = config.get_system_prompt();
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (system_prompt, _) = assemble_system_prompt(&base_prompt, &cwd, bare);
        let api_key = config.get_api_key(provider).unwrap_or_default();
        let mcp = if bare && !opts.explicit_mcp {
            Vec::new()
        } else {
            config.get_mcp()
        };

        AgentConfig {
            provider: provider.unwrap_or_default(),
            model,
            base_url,
            skills,
            system_prompt,
            api_key,
            mcp,
        }
    }
}

impl From<&Config> for AgentConfig {
    fn from(config: &Config) -> Self {
        Self::from_config(config, AgentConfigOptions::default())
    }
}

#[derive(Default, Deref, Resource)]
pub struct AgentsCancelToken(Arc<CancellationToken>);

fn setup(
    config: Res<Config>,
    cli: Res<crate::cli::Cli>,
    mut commands: Commands,
    app_cancel: Res<AppCancelToken>,
    agents_cancel: Res<AgentsCancelToken>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
) {
    let agent_config = AgentConfig::from_config(
        config.into_inner(),
        AgentConfigOptions::from_cli(cli.as_ref()),
    );
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
