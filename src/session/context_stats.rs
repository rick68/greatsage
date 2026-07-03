use {
    super::{
        components::{SessionContextStats, SessionId},
        resources::SessionManager,
    },
    bevy::ecs::{resource::Resource, world::World},
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

/// Apply restored yoagent messages to `SessionContextStats` after `/load` or `--continue`.
pub(crate) fn sync_context_stats_on_world(
    world: &mut World,
    session_id: SessionId,
    messages: &[AgentMessage],
    context_max: u64,
) {
    let Some(root) = world.resource::<SessionManager>().root_entity(session_id) else {
        return;
    };
    let mut query = world.query::<&mut SessionContextStats>();
    let Ok(mut stats) = query.get_mut(world, root) else {
        return;
    };
    () = apply_context_stats(&mut stats, messages, context_max);
}
