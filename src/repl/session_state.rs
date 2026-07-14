//! Per-REPL-session state (e.g. last user prompt for `/retry`, in-memory bookmarks).

use bevy::{ecs::resource::Resource, platform::collections::HashMap};

#[derive(Default, Resource)]
pub(crate) struct ReplSessionState {
    pub last_user_prompt: Option<String>,
    pub pending_clear_confirm: bool,
    /// Named yoagent `save_messages()` JSON snapshots for `/mark` / `/jump` / `/marks`.
    /// Session-scoped only — not persisted across process exit.
    pub bookmarks: HashMap<String, String>,
}
