//! TUI Bevy plugin registration — **must** use `bevy_ratatui::RatatuiPlugins`.

use {
    super::{
        auth_ui::{AuthUiChannel, poll_auth_ui_system},
        draw::draw_system,
        input::{input_system, mouse_input_system, paste_system, poll_shell_system},
        scrollback::{ScrollbackView, rebuild_scrollback_view},
        state::TuiState,
    },
    crate::repl::session_state::ReplSessionState,
    bevy::{
        app::{App, Plugin, PostUpdate, PreUpdate, Startup, Update},
        ecs::{resource::Resource, schedule::IntoScheduleConfigs, system::Commands},
        utils::default,
    },
    bevy_ratatui::{
        RatatuiPlugins,
        crossterm::{
            ExecutableCommand,
            event::{DisableBracketedPaste, EnableBracketedPaste},
        },
    },
    std::io::stdout,
};

pub struct TuiPlugin;

impl Plugin for TuiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RatatuiPlugins {
            // Required for terminal to emit mouse events (click focus, wheel scroll).
            enable_mouse_capture: true,
            ..default()
        })
        .init_resource::<TuiState>()
        .init_resource::<ScrollbackView>()
        .init_resource::<ReplSessionState>()
        .init_resource::<AuthUiChannel>()
        .add_systems(
            Startup,
            (enable_bracketed_paste_system, install_paste_cleanup),
        )
        .add_systems(
            PreUpdate,
            (
                poll_shell_system,
                poll_auth_ui_system,
                paste_system,
                input_system,
                mouse_input_system,
            )
                .chain(),
        )
        .add_systems(Update, rebuild_scrollback_view)
        .add_systems(PostUpdate, draw_system);
    }
}

/// Bracketed paste: multi-char IME commits / paste arrive as `PasteMessage` (not key spam).
fn enable_bracketed_paste_system() {
    let _ = stdout().execute(EnableBracketedPaste);
}

fn install_paste_cleanup(mut commands: Commands) {
    commands.insert_resource(TuiPasteGuardResource);
}

/// Disables bracketed paste when the TUI world is dropped.
#[derive(Resource)]
struct TuiPasteGuardResource;

impl Drop for TuiPasteGuardResource {
    fn drop(&mut self) {
        let _ = stdout().execute(DisableBracketedPaste);
    }
}

/// Register the full-screen TUI stack (exclusive with line REPL plugins).
pub fn tui_plugin(app: &mut App) {
    app.add_plugins(TuiPlugin);
}
