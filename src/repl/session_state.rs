//! Per-REPL-session state (e.g. last user prompt for `/retry`, in-memory bookmarks).

use {
    super::{
        shell_bg::BackgroundJobTracker,
        shell_run::{ActiveShellHandle, LastFailedRun},
    },
    bevy::{ecs::resource::Resource, platform::collections::HashMap},
};

#[derive(Default, Resource)]
pub(crate) struct ReplSessionState {
    pub last_user_prompt: Option<String>,
    pub pending_clear_confirm: bool,
    /// Named yoagent `save_messages()` JSON snapshots for `/mark` / `/jump` / `/marks`.
    /// Session-scoped only — not persisted across process exit.
    pub bookmarks: HashMap<String, String>,
    /// Last non-zero `/run` / `!` result (for a future yoyo-aligned `/fix` / health change).
    /// Process-scoped only — not persisted across process exit.
    pub last_failed_run: Option<LastFailedRun>,
    /// In-flight `/run` / `!` (worker thread); Ctrl+C interrupts without AppExit.
    pub active_shell: Option<ActiveShellHandle>,
    /// Multi-job `/bg` tracker (independent of [`Self::active_shell`]).
    pub bg_jobs: BackgroundJobTracker,
    /// After any first Ctrl+C (shell / agent / idle cancel-line): next Ctrl+C Leaves app.
    /// Cleared on new prompt, normal typing, or Leave app.
    pub ctrl_c_armed: bool,
    /// Leave app in progress — skip duplicate farewell and post-agent `>` redraw.
    pub leaving: bool,
}
