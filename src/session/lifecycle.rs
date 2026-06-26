use {
    super::{
        components::{
            SessionId, SessionIngestState, SessionMeta, SessionRuntimeStatus, SessionSeq,
        },
        resources::SessionManager,
    },
    crate::utils::now_ms,
    bevy::ecs::{entity::Entity, world::World},
    std::env,
};

/// Spawn a session root entity and register it with `SessionManager`.
pub(crate) fn spawn_session_root(world: &mut World) -> (SessionId, Entity) {
    let session_id = {
        let mut manager = world.resource_mut::<SessionManager>();
        manager.allocate()
    };
    let entity = world
        .spawn((
            session_id,
            SessionSeq::default(),
            SessionMeta::default(),
            SessionIngestState::default(),
            SessionRuntimeStatus::default(),
        ))
        .id();
    () = world
        .resource_mut::<SessionManager>()
        .register_root(session_id, entity);
    (session_id, entity)
}

/// Remove a session from the registry and despawn its root subtree.
pub(crate) fn teardown_session(world: &mut World, session_id: SessionId) {
    let root = {
        let mut manager = world.resource_mut::<SessionManager>();
        manager.unregister(session_id)
    };
    if let Some(root) = root {
        world.despawn(root);
    }
}

/// Populate session root metadata once agent config is available.
pub(crate) fn sync_session_meta(
    world: &mut World,
    session_id: SessionId,
    model: String,
    provider: String,
) {
    let Some(root) = world.resource::<SessionManager>().root_entity(session_id) else {
        return;
    };
    let cwd = env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_default();

    world.entity_mut(root).insert(SessionMeta {
        session_id,
        started_at_ms: now_ms(),
        cwd,
        model,
        provider,
    });
}
