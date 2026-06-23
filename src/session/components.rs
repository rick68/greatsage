#[cfg(feature = "dev_native")]
use bevy::prelude::{Reflect, ReflectComponent, ReflectDefault};
use {
    bevy::{
        ecs::entity::Entity,
        platform::collections::{HashMap, HashSet},
        prelude::Component,
    },
    std::{mem, sync::Arc},
    yoagent::types::Content,
};

/// Where a projected `Content` block originated in the yoagent event stream.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ContentProvenance {
    UserMessage {
        timestamp_ms: u64,
    },
    AssistantMessage {
        timestamp_ms: u64,
    },
    ToolExecutionStart {
        tool_call_id: String,
        tool_name: String,
    },
    TurnToolResult {
        tool_call_id: String,
        tool_name: String,
        is_error: bool,
        timestamp_ms: u64,
    },
}

impl ContentProvenance {
    pub(crate) fn source_timestamp_ms(&self) -> u64 {
        match self {
            Self::UserMessage { timestamp_ms }
            | Self::AssistantMessage { timestamp_ms }
            | Self::TurnToolResult { timestamp_ms, .. } => *timestamp_ms,
            Self::ToolExecutionStart { .. } => 0,
        }
    }

    pub(crate) fn tool_call_id(&self) -> Option<&str> {
        match self {
            Self::ToolExecutionStart { tool_call_id, .. }
            | Self::TurnToolResult { tool_call_id, .. } => Some(tool_call_id),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IndexedContent {
    pub(crate) block_index: u32,
    pub(crate) content: Content,
    pub(crate) provenance: ContentProvenance,
}

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
    pending_user_content: Vec<IndexedContent>,
    /// Assistant text accumulated from `MessageUpdate` until turn boundary.
    pending_assistant_text: String,
    invocation_user_prompt_consumed: bool,
    /// Tool call IDs whose result blocks were already projected from `TurnEnd.tool_results`.
    projected_turn_tool_results: HashSet<String>,
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

    pub(crate) fn clear_turn(&mut self) {
        self.current_turn = None;
        () = self.tool_entities.clear();
    }

    pub(crate) fn finish_turn(&mut self) {
        self.current_turn = None;
        () = self.tool_entities.clear();
    }

    pub(crate) fn reset_invocation(&mut self) {
        () = self.pending_user_content.clear();
        self.pending_assistant_text.clear();
        self.invocation_user_prompt_consumed = false;
        () = self.projected_turn_tool_results.clear();
    }

    pub(crate) fn append_assistant_text(&mut self, delta: &str) {
        self.pending_assistant_text.push_str(delta);
    }

    pub(crate) fn take_pending_assistant_text(&mut self) -> String {
        mem::take(&mut self.pending_assistant_text)
    }

    pub(crate) fn set_pending_user_content(&mut self, blocks: Vec<IndexedContent>) {
        self.pending_user_content = blocks;
    }

    pub(crate) fn take_user_content_for_turn(&mut self) -> Vec<IndexedContent> {
        if self.invocation_user_prompt_consumed {
            return Vec::new();
        }
        self.invocation_user_prompt_consumed = true;
        mem::take(&mut self.pending_user_content)
    }

    pub(crate) fn mark_turn_tool_result_projected(&mut self, tool_call_id: &str) {
        self.projected_turn_tool_results
            .insert(tool_call_id.to_string());
    }

    pub(crate) fn turn_tool_result_already_projected(&self, tool_call_id: &str) -> bool {
        self.projected_turn_tool_results.contains(tool_call_id)
    }
}

/// One yoagent `Content` block projected for BRP/debug inspection.
///
/// `content_json` holds the native `yoagent::types::Content` value (serde).
/// Other string fields are denormalized for cheap BRP filtering.
#[allow(dead_code)]
#[derive(Clone, Component, Debug)]
#[cfg_attr(feature = "dev_native", derive(Reflect), reflect(Component))]
pub(crate) struct ContentBlock {
    pub(crate) seq: u64,
    pub(crate) block_index: u32,
    pub(crate) recorded_at_ms: u64,
    pub(crate) source_timestamp_ms: u64,
    pub(crate) provenance_kind: String,
    pub(crate) content_kind: String,
    pub(crate) content_text: String,
    pub(crate) content_json: String,
    pub(crate) tool_call_id: Option<String>,
    pub(crate) tool_name: Option<String>,
    pub(crate) is_error: Option<bool>,
}

/// Marker for content-block entities (queryable separately from turns/tools).
#[derive(Clone, Component, Copy, Debug, Default)]
pub(crate) struct ContentBlockEntity;

/// Per-turn token usage projected from `TurnEnd` (or `AgentEnd` fallback).
#[allow(dead_code)]
#[derive(Clone, Component, Debug)]
#[cfg_attr(feature = "dev_native", derive(Reflect), reflect(Component))]
pub(crate) struct TurnSummary {
    pub(crate) seq: u64,
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) cache_read_tokens: u64,
    pub(crate) cache_write_tokens: u64,
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
