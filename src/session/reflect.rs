//! Registers session types with Bevy Reflect for `dev_native` BRP queries.
//!
//! Adding new Reflect types may require restarting `scripts/dx_serve.sh` — hot-patch
//! does not always register newly introduced reflection metadata.

use {
    super::components::{
        AgentRuntimeState, SessionId, SessionMeta, SessionRuntimeStatus, ToolCallRecord,
        TurnSummary,
    },
    bevy::app::App,
};

pub(crate) fn register_session_reflect(app: &mut App) {
    app.register_type::<AgentRuntimeState>()
        .register_type::<SessionRuntimeStatus>()
        .register_type::<SessionId>()
        .register_type::<SessionMeta>()
        .register_type::<TurnSummary>()
        .register_type::<ToolCallRecord>();
}
