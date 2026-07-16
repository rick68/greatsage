//! UI-only command helpers (quit, palette actions, theme); agent slash stays in `repl` dispatch.

use {
    super::{
        auth_ui::{AuthUiChannel, panel_auth_logout, panel_auth_status, spawn_device_login},
        palette::{PaletteAccept, PaletteUiAction, accept_selected},
        state::TuiState,
        theme::ThemeId,
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

/// Whether the line is `/theme` or `/t` (with optional args).
pub fn is_theme_slash(line: &str) -> bool {
    let cmd = line
        .split_whitespace()
        .next()
        .unwrap_or(line)
        .trim()
        .to_ascii_lowercase();
    matches!(cmd.as_str(), "/theme" | "/t")
}

/// Handle `/theme` / `/t` under TUI. Returns `true` if handled (caller skips agent dispatch).
///
/// - bare → cycle next catalog id, apply + persist
/// - name → apply if known; else error cue, keep prior
pub fn handle_theme_slash(line: &str, state: &mut TuiState, config: &Config) -> bool {
    if !is_theme_slash(line) {
        return false;
    }
    let mut parts = line.split_whitespace();
    let _cmd = parts.next();
    let arg = parts.next().unwrap_or("").trim();
    if arg.is_empty() {
        let next = state.active_theme_id.next();
        () = state.apply_theme_id(next);
        () = config.set_theme(next.as_str());
        return true;
    }
    if !ThemeId::is_known_name(arg) {
        () =state.set_status_hint(format!("unknown theme: {arg}"));
        return true;
    }
    let id = ThemeId::resolve(arg);
    () = state.apply_theme_id(id);
    () = config.set_theme(id.as_str());
    true
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
        PaletteAccept::Ui(PaletteUiAction::OpenThemePicker) => {
            () = state.open_theme_picker();
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
