use {
    super::components::SessionId,
    bevy::{
        ecs::{entity::Entity, resource::Resource},
        platform::collections::HashMap,
    },
};

#[derive(Debug, Default, Resource)]
pub(crate) struct SessionManager {
    roots: HashMap<SessionId, Entity>,
    next_id: u64,
}

impl SessionManager {
    pub(crate) fn allocate(&mut self) -> SessionId {
        let id = SessionId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    pub(crate) fn root_entity(&self, id: SessionId) -> Option<Entity> {
        self.roots.get(&id).copied()
    }

    pub(crate) fn register_root(&mut self, id: SessionId, entity: Entity) {
        self.roots.insert(id, entity);
    }

    pub(crate) fn unregister(&mut self, id: SessionId) -> Option<Entity> {
        self.roots.remove(&id)
    }

    pub(crate) fn session_ids(&self) -> impl Iterator<Item = SessionId> + '_ {
        self.roots.keys().copied()
    }
}

/// UI / REPL focus — which session the operator is interacting with (not for event routing).
#[derive(Debug, Default, Resource)]
pub(crate) struct FocusedSession(pub(crate) Option<SessionId>);

impl FocusedSession {
    pub(crate) fn set(&mut self, id: SessionId) {
        self.0 = Some(id);
    }
}

#[derive(Debug, Resource)]
pub(crate) struct SessionLimits {
    max_turns: usize,
    max_tool_records: usize,
}

impl Default for SessionLimits {
    fn default() -> Self {
        Self {
            max_turns: 200,
            max_tool_records: 2_000,
        }
    }
}

impl SessionLimits {
    #[allow(dead_code)] // used by external test harness (`tests/session_prune.rs`)
    pub(crate) fn with_limits(max_turns: usize, max_tool_records: usize) -> Self {
        Self {
            max_turns,
            max_tool_records,
        }
    }

    pub(crate) fn max_turns(&self) -> usize {
        self.max_turns
    }

    pub(crate) fn max_tool_records(&self) -> usize {
        self.max_tool_records
    }
}
