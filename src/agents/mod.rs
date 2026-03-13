mod coding;
pub use coding::CodingAgentRequest;

mod tools;

use {
    self::coding::coding_agent_plugin,
    autoagents::{
        core::runtime::SingleThreadedRuntime,
        llm::{LLMProvider, backends::openai::OpenAI, builder::LLMBuilder},
        llm_error::LLMError,
    },
    bevy::{
        app::{App, PreStartup},
        ecs::{
            error::BevyError, resource::Resource, schedule::IntoScheduleConfigs, system::Commands,
        },
        prelude::{Deref, DerefMut},
    },
    std::sync::Arc,
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

pub fn agents_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<AgentsCancelToken>()
        .add_plugins::<_>(coding_agent_plugin)
        .add_systems::<_>(PreStartup, (llm_setup, runtime_setup).chain());
}
