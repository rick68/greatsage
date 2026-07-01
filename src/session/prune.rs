use {
    super::{
        components::{SessionId, ToolCallRecord, TurnEntity, TurnSummary},
        resources::{SessionLimits, SessionManager},
    },
    bevy::prelude::{Commands, Entity, Query, Res, With},
};

/// When `seqs.len() > max`, return the seq threshold: entities with `seq <= cutoff` are the
/// oldest excess and should be despawned. Uses `select_nth_unstable` (no full sort).
fn seq_cutoff(seqs: &mut [u64], max: usize) -> Option<u64> {
    if seqs.len() <= max {
        return None;
    }
    let cutoff_idx = seqs.len() - max - 1;
    seqs.select_nth_unstable(cutoff_idx);
    Some(seqs[cutoff_idx])
}

fn prune_turns(
    session_id: SessionId,
    limits: &SessionLimits,
    commands: &mut Commands,
    turns: &Query<(Entity, &SessionId, &TurnSummary), With<TurnEntity>>,
) {
    let mut seqs: Vec<u64> = turns
        .iter()
        .filter(|(_, id, _)| **id == session_id)
        .map(|(_, _, summary)| summary.seq)
        .collect();

    let Some(cutoff) = seq_cutoff(&mut seqs, limits.max_turns()) else {
        return;
    };

    for (entity, id, summary) in turns.iter() {
        if *id == session_id && summary.seq <= cutoff {
            // `ChildOf` children are despawned automatically when the parent goes away.
            () = commands.entity(entity).despawn();
        }
    }
}

fn prune_tools(
    session_id: SessionId,
    limits: &SessionLimits,
    commands: &mut Commands,
    tools: &Query<(Entity, &SessionId, &ToolCallRecord)>,
) {
    let mut seqs: Vec<u64> = tools
        .iter()
        .filter(|(_, id, _)| **id == session_id)
        .map(|(_, _, record)| record.seq)
        .collect();

    let Some(cutoff) = seq_cutoff(&mut seqs, limits.max_tool_records()) else {
        return;
    };

    for (entity, id, record) in tools.iter() {
        if *id == session_id && record.seq <= cutoff {
            () = commands.entity(entity).despawn();
        }
    }
}

pub(crate) fn prune_session(
    limits: Res<SessionLimits>,
    session_manager: Res<SessionManager>,
    mut commands: Commands,
    turns: Query<(Entity, &SessionId, &TurnSummary), With<TurnEntity>>,
    tools: Query<(Entity, &SessionId, &ToolCallRecord)>,
) {
    for session_id in session_manager.session_ids().collect::<Vec<_>>() {
        if session_manager.root_entity(session_id).is_none() {
            continue;
        }
        () = prune_turns(session_id, &limits, &mut commands, &turns);
        () = prune_tools(session_id, &limits, &mut commands, &tools);
    }
}
