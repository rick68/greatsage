//! UI-only command helpers (quit, palette actions); agent slash stays in `repl` dispatch.

use {
    super::{
        auth_ui::{AuthUiChannel, panel_auth_logout, panel_auth_status, spawn_device_login},
        palette::{PaletteAccept, PaletteUiAction, accept_selected},
        state::TuiState,
    },
    crate::config::Config,
    bevy_tokio_tasks::TokioTasksRuntime,
};

/// Returns true when the line is a quit request that should exit the TUI.
pub fn is_ui_quit_line(line: &str) -> bool {
    matches!(
        line.trim(),
        "/quit" | "/exit" | "/q" | "quit" | "exit" | ":q"
    )
}

/// Apply the highlighted palette row. Returns `true` if a row was accepted.
///
/// Slash rows fill the draft only — never dispatches. UI rows run local chrome.
pub fn accept_palette_selection(
    state: &mut TuiState,
    config: &Config,
    runtime: &TokioTasksRuntime,
    auth_ui: &AuthUiChannel,
) -> bool {
    let Some(action) = accept_selected(&state.command_palette) else {
        return false;
    };
    state.close_command_palette();
    match action {
        PaletteAccept::FillSlash(cmd) => {
            state.prompt = cmd;
            state.cursor = state.prompt.len();
            state.slash_menu = super::state::SlashMenuState::default();
            state.ghost_hint = None;
            state.focus = super::state::TuiFocus::Prompt;
            true
        }
        PaletteAccept::Ui(PaletteUiAction::OpenShortcuts) => {
            () = state.open_shortcuts_cheatsheet();
            true
        }
        PaletteAccept::Ui(PaletteUiAction::ClearPrompt) => {
            () = state.clear_prompt();
            () = state.clear_esc_arm();
            // Restore default key chrome (same as 2×Esc clear).
            state.status_hint = None;
            true
        }
        PaletteAccept::Ui(PaletteUiAction::AuthStatus) => {
            let (title, lines) = panel_auth_status(config);
            () = state.open_operator_panel_readonly(title, lines);
            true
        }
        PaletteAccept::Ui(PaletteUiAction::AuthLogout) => {
            let (title, lines) = panel_auth_logout(config);
            () = state.open_operator_panel_readonly(title, lines);
            true
        }
        PaletteAccept::Ui(PaletteUiAction::AuthLogin) => {
            // Device login on app runtime; URL/code → OperatorPanel via AuthUiChannel.
            () = spawn_device_login(config, runtime, auth_ui, true);
            true
        }
    }
}
