//! Per-REPL-session state (e.g. last user prompt for `/retry`).

use bevy::ecs::resource::Resource;

#[derive(Default, Resource)]
pub(crate) struct ReplSessionState {
    pub last_user_prompt: Option<String>,
    pub pending_clear_confirm: bool,
}
