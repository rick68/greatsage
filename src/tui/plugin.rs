//! TUI Bevy plugin registration — **must** use `bevy_ratatui::RatatuiPlugins`.

use {
    super::{
        draw::draw_system,
        input::{input_system, mouse_input_system, poll_shell_system},
        scrollback::{ScrollbackView, rebuild_scrollback_view},
        state::TuiState,
    },
    crate::repl::session_state::ReplSessionState,
    bevy::{
        app::{App, Plugin, PostUpdate, PreUpdate, Update},
        ecs::schedule::IntoScheduleConfigs,
        utils::default,
    },
    bevy_ratatui::RatatuiPlugins,
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
        .add_systems(
            PreUpdate,
            (poll_shell_system, input_system, mouse_input_system).chain(),
        )
        .add_systems(Update, rebuild_scrollback_view)
        .add_systems(PostUpdate, draw_system);
    }
}

/// Register the full-screen TUI stack (exclusive with line REPL plugins).
pub fn tui_plugin(app: &mut App) {
    app.add_plugins(TuiPlugin);
}
