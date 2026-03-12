mod coding;
pub use coding::CodingAgentRequest;

mod tools;

use {
    self::coding::coding_agent_plugin,
    autoagents::llm::LLMProvider,
    bevy::{
        app::App,
        ecs::{message::Message, resource::Resource},
        prelude::{Deref, DerefMut},
    },
    std::sync::Arc,
    tokio_util::sync::CancellationToken,
};

const MAX_TOKENS: u32 = 131_072;
const SLIDING_WINDOW_MEMORY: usize = 300;
const MAX_TURNS: usize = 10;

#[derive(Deref, DerefMut, Resource)]
struct Llm(Arc<dyn LLMProvider>);

#[derive(Deref, DerefMut, Resource)]
struct GlobalAgentRuntime(Arc<SingleThreadedRuntime>);

#[derive(Deref, DerefMut, Message)]
struct ProtocolEvent(Event);

#[derive(Default, Deref, Resource)]
struct AgentsCancelToken(Arc<CancellationToken>);

pub fn agents_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<AgentsCancelToken>()
        .add_plugins::<_>(coding_agent_plugin);
}
