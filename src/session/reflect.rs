//! Registers session types with Bevy Reflect for `dev_native` BRP queries.
//!
//! Schema changes to `ContentBlock`, `TurnSummary`, or `ToolCallRecord` require
//! restarting `scripts/dx_serve.sh` — hot-patch does not always register new
//! reflection metadata. `SessionIngestState` stays unregistered (internal cursors).

use {
    super::components::{
        AgentRuntimeState, ContentBlock, SessionId, SessionMeta, SessionRuntimeStatus,
        ToolCallRecord, TurnSummary,
    },
    bevy::app::App,
};

pub(crate) fn register_session_reflect(app: &mut App) {
    app.register_type::<AgentRuntimeState>()
        .register_type::<SessionRuntimeStatus>()
        .register_type::<SessionId>()
        .register_type::<SessionMeta>()
        .register_type::<ContentBlock>()
        .register_type::<TurnSummary>()
        .register_type::<ToolCallRecord>();
}
