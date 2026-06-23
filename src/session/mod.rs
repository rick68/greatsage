mod components;
pub(crate) use components::{AgentId, SessionId, SessionRuntimeStatus};

mod lifecycle;
pub(crate) use lifecycle::{spawn_session_root, sync_session_meta, teardown_session};

mod ingest;
mod prune;
#[cfg(feature = "dev_native")]
mod reflect;
mod resources;
pub(crate) use resources::{FocusedSession, SessionManager};

use {
    self::{ingest::ingest_agent_events, prune::prune_session, resources::SessionLimits},
    bevy::{
        app::{App, Update},
        ecs::schedule::{IntoScheduleConfigs, common_conditions::resource_exists},
    },
};

use crate::agents::CodingAgentTask;

pub fn session_plugin(app: &mut App) {
    #[cfg(feature = "dev_native")]
    reflect::register_session_reflect(app);

    app.init_resource::<SessionManager>()
        .init_resource::<SessionLimits>()
        .init_resource::<FocusedSession>()
        .add_systems(
            Update,
            (ingest_agent_events, prune_session).run_if(resource_exists::<CodingAgentTask>),
        );
}
