mod coding;
pub use coding::CodingAgentRequest;

mod tools;

use {
    self::coding::coding_agent_plugin,
    autoagents::llm::LLMProvider,
    bevy::{
        app::App,
        ecs::resource::Resource,
        prelude::{Deref, DerefMut},
    },
    std::sync::Arc,
    tokio_util::sync::CancellationToken,
};

#[derive(Deref, DerefMut, Resource)]
struct Llm(Arc<dyn LLMProvider>);

#[derive(Default, Deref, Resource)]
struct AgentsCancelToken(Arc<CancellationToken>);

pub fn agents_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<AgentsCancelToken>()
        .add_plugins::<_>(coding_agent_plugin);
}
