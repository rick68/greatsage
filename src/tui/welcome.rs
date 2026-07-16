//! Ephemeral empty-session / welcome presentation for TUI scrollback.
//!
//! Drawn only when Session ECS has no projectable blocks — never inserted as
//! `ContentBlock` entities (see `TuiEmptyWelcomePresentation`).

/// Multi-line empty-session chrome (title + key hints + type-a-prompt cue).
pub fn welcome_lines() -> Vec<String> {
    vec![
        String::from("greatsage"),
        String::new(),
        String::from("Ctrl+P commands · Ctrl+X keys · / menu"),
        String::from("empty ↑ prompt history · Tab focus"),
        String::new(),
        String::from("type a prompt below"),
    ]
}

/// True when the welcome body is non-empty (contract for tests / draw guards).
#[allow(dead_code)] // pure contract helper; draw uses `welcome_lines` directly
pub fn welcome_is_presentable() -> bool {
    welcome_lines().iter().any(|l| !l.is_empty())
}
