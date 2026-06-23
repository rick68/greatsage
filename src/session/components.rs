#[cfg(feature = "dev_native")]
use bevy::prelude::{Reflect, ReflectComponent, ReflectDefault};
use {
    bevy::{ecs::entity::Entity, platform::collections::HashMap, prelude::Component},
    std::sync::Arc,
};

/// Process-lifetime identity of a live `Arc<Mutex<Agent>>` allocation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AgentId(pub usize);

impl AgentId {
    pub fn of<T: ?Sized>(arc: &Arc<T>) -> Self {
        Self(Arc::as_ptr(arc) as *const () as usize)
    }
}

/// User-facing session number allocated monotonically by `SessionManager`.
#[derive(Clone, Component, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "dev_native", derive(Reflect))]
#[cfg_attr(feature = "dev_native", reflect(Component))]
pub struct SessionId(pub u64);

/// Non-secret session metadata on the session root entity.
///
/// Fields are written by ingest/setup and read by BRP `world_query`, not in-process Rust.
#[allow(dead_code)]
#[derive(Debug, Clone, Component)]
#[cfg_attr(feature = "dev_native", derive(Reflect), reflect(Component))]
pub(crate) struct SessionMeta {
    pub(crate) session_id: SessionId,
    pub(crate) started_at_ms: u64,
    pub(crate) cwd: String,
    pub(crate) model: String,
    pub(crate) provider: String,
}

impl Default for SessionMeta {
    fn default() -> Self {
        Self {
            session_id: SessionId::default(),
            started_at_ms: 0,
            cwd: String::new(),
            model: String::new(),
            provider: String::new(),
        }
    }
}

/// Monotonic sequence counter for turn and tool records within a session.
#[derive(Clone, Component, Copy, Debug, Default)]
pub(crate) struct SessionSeq {
    next: u64,
}

impl SessionSeq {
    pub(crate) fn bump(&mut self) -> u64 {
        let value = self.next;
        self.next = self.next.saturating_add(1);
        value
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "dev_native", derive(Reflect), reflect(Default))]
pub(crate) enum AgentRuntimeState {
    #[default]
    Idle,
    Processing,
}

/// Per-session agent idle/processing state on the session root entity.
#[derive(Clone, Component, Debug, Default)]
#[cfg_attr(feature = "dev_native", derive(Reflect), reflect(Component))]
pub(crate) struct SessionRuntimeStatus {
    state: AgentRuntimeState,
}

impl SessionRuntimeStatus {
    pub(crate) fn processing() -> Self {
        Self {
            state: AgentRuntimeState::Processing,
        }
    }

    pub(crate) fn set_processing(&mut self) {
        self.state = AgentRuntimeState::Processing;
    }

    pub(crate) fn set_idle(&mut self) {
        self.state = AgentRuntimeState::Idle;
    }
}

/// Per-session ingest cursor on the session root entity (not reflected — no secrets).
#[derive(Component, Debug, Default)]
pub(crate) struct SessionIngestState {
    current_turn: Option<Entity>,
    turn_recorded: bool,
    tool_entities: HashMap<String, Entity>,
}

impl SessionIngestState {
    pub(crate) fn current_turn(&self) -> Option<Entity> {
        self.current_turn
    }

    pub(crate) fn set_current_turn(&mut self, entity: Option<Entity>) {
        self.current_turn = entity;
    }

    pub(crate) fn take_current_turn(&mut self) -> Option<Entity> {
        self.current_turn.take()
    }

    pub(crate) fn turn_recorded(&self) -> bool {
        self.turn_recorded
    }

    pub(crate) fn set_turn_recorded(&mut self, recorded: bool) {
        self.turn_recorded = recorded;
    }

    pub(crate) fn track_tool(&mut self, tool_call_id: String, entity: Entity) {
        self.tool_entities.insert(tool_call_id, entity);
    }

    pub(crate) fn tool_entity(&self, tool_call_id: &str) -> Option<Entity> {
        self.tool_entities.get(tool_call_id).copied()
    }

    pub(crate) fn remove_tool(&mut self, tool_call_id: &str) {
        self.tool_entities.remove(tool_call_id);
    }

    pub(crate) fn clear_turn(&mut self) {
        self.current_turn = None;
        () = self.tool_entities.clear();
    }

    pub(crate) fn finish_turn(&mut self) {
        self.current_turn = None;
        () = self.tool_entities.clear();
    }
}

/// Per-turn token usage projected from `TurnEnd` (or `AgentEnd` fallback).
#[allow(dead_code)]
#[derive(Clone, Component, Debug)]
#[cfg_attr(feature = "dev_native", derive(Reflect), reflect(Component))]
pub(crate) struct TurnSummary {
    pub(crate) seq: u64,
    pub(crate) input: u64,
    pub(crate) output: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_write: u64,
    pub(crate) ended_at_ms: u64,
}

/// Marker for turn entities (may exist before `TurnSummary` is attached).
#[derive(Clone, Component, Copy, Debug, Default)]
pub(crate) struct TurnEntity;

/// Tool execution record projected from yoagent tool events.
#[allow(dead_code)]
#[derive(Clone, Component, Debug)]
#[cfg_attr(feature = "dev_native", derive(Reflect), reflect(Component))]
pub(crate) struct ToolCallRecord {
    pub(crate) seq: u64,
    pub(crate) tool_call_id: String,
    pub(crate) tool_name: String,
    pub(crate) summary: String,
    pub(crate) started_at_ms: u64,
    pub(crate) ended_at_ms: Option<u64>,
    pub(crate) is_error: bool,
}

/// Marks a tool entity as currently executing.
#[derive(Clone, Component, Copy, Debug, Default)]
pub(crate) struct ActiveToolCall;
