//! UI-only command helpers (quit, palette actions); agent slash stays in `repl` dispatch.

use super::{
    palette::{PaletteAccept, PaletteUiAction, accept_selected},
    state::TuiState,
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
pub fn accept_palette_selection(state: &mut TuiState) -> bool {
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
            () = state.set_status_hint("prompt cleared");
            true
        }
    }
}
