mod components;
pub(crate) use components::{
    AgentId, SessionContextStats, SessionId, SessionMeta, SessionRuntimeStatus, TurnEntity,
    TurnSummary,
};

#[cfg(feature = "dev_native")]
mod brp;
#[cfg(feature = "dev_native")]
pub use brp::register_session_brp_methods;

mod content;

mod lifecycle;
pub(crate) use lifecycle::{spawn_session_root, sync_session_meta, teardown_session};

pub(crate) mod context_stats;
pub(crate) mod ingest;
mod prune;

#[cfg(feature = "dev_native")]
mod reflect;

mod resources;
pub(crate) use resources::{FocusedSession, SessionLifetimeUsage, SessionManager};

use {
    self::{ingest::ingest_agent_events, prune::prune_session, resources::SessionLimits},
    crate::{agents::CodingAgentTask, config::Config},
    bevy::{
        app::{App, PostUpdate, Startup, Update},
        ecs::{
            change_detection::Res,
            schedule::{IntoScheduleConfigs, common_conditions::resource_exists},
            system::Commands,
        },
    },
};

fn init_session_limits(config: Res<Config>, mut commands: Commands) {
    let (max_turns, max_tool_records) = config.session_limits();
    () = commands.insert_resource(SessionLimits::with_limits(max_turns, max_tool_records));
}

pub fn session_plugin(app: &mut App) {
    #[cfg(feature = "dev_native")]
    reflect::register_session_reflect(app);

    app.init_resource::<SessionManager>()
        .init_resource::<FocusedSession>()
        .init_resource::<SessionLifetimeUsage>()
        .init_resource::<context_stats::PendingContextStatsSync>()
        // After `config::setup` (PreStartup) inserts `Config`.
        .add_systems(Startup, init_session_limits)
        // Ingest must run after `CodingAgentTask` is removed so final `TurnEnd` /
        // `AgentEnd` messages are projected before the process exits.
        .add_systems(PostUpdate, ingest_agent_events)
        .add_systems(
            Update,
            prune_session.run_if(resource_exists::<CodingAgentTask>),
        );
}
