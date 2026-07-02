use {
    super::components::{SessionContextStats, SessionId},
    bevy::ecs::resource::Resource,
    yoagent::{context::total_tokens, types::AgentMessage},
};

/// Apply active-context fields from a yoagent message snapshot.
pub(crate) fn apply_context_stats(
    stats: &mut SessionContextStats,
    messages: &[AgentMessage],
    context_max: u64,
) {
    stats.message_count = messages.len() as u32;
    stats.context_used = total_tokens(messages) as u64;
    stats.context_max = context_max;
}

/// Set when ingest records turn usage; drained by `sync_pending_session_context_stats`.
#[derive(Debug, Default, Resource)]
pub(crate) struct PendingContextStatsSync(pub(crate) Option<SessionId>);

pub(crate) fn request_context_stats_sync(
    pending: &mut PendingContextStatsSync,
    session_id: SessionId,
) {
    pending.0 = Some(session_id);
}
