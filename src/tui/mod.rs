pub mod commands;
pub mod core;
pub mod events;
pub mod input;
pub mod renderer;

pub use self::core::{TuiMain, TuiMainFocus};
pub use self::events::RenderNeeded;

#[cfg(test)]
mod tests;

use {
    crate::tui::events::{TuiAction, TuiCommandEvent},
    bevy::{
        app::{App, AppExit, PreUpdate, Startup, Update},
        ecs::{
            change_detection::ResMut,
            message::MessageReader,
            schedule::{IntoScheduleConfigs, common_conditions::resource_exists},
        },
        state::{app::AppExtStates, condition::in_state},
        utils::default,
    },
    bevy_ratatui::{
        RatatuiPlugins,
        crossterm::{cursor::SetCursorStyle, execute},
        event::ResizeMessage,
    },
    std::io::stdout,
};

/// Handles window resize events, marking the UI as needing a redraw.
fn handle_resize(mut messages: MessageReader<ResizeMessage>, mut dirty: ResMut<RenderNeeded>) {
    if let Some(ResizeMessage(_size)) = messages.read().next() {
        **dirty = true;
    }
}

/// Initializes the terminal cursor style.
fn setup_cursor() {
    let _ = execute!(stdout(), SetCursorStyle::SteadyBlock);
}

/// TUI Main Plugin, responsible for assembling all sub-modules.
pub fn tui_plugin(app: &mut App) {
    app
        // Initialize resources and state
        .init_resource::<RenderNeeded>()
        .init_non_send_resource::<TuiMain>()
        .init_state::<TuiMainFocus>()
        .add_message::<TuiAction>()
        .add_message::<TuiCommandEvent>()
        .add_message::<AppExit>()
        // Load external plugins
        .add_plugins(RatatuiPlugins {
            enable_mouse_capture: true,
            enable_input_forwarding: true,
            ..default::<RatatuiPlugins>()
        })
        // Register startup systems
        .add_systems(Startup, setup_cursor)
        // Register input handling systems
        .add_systems(
            PreUpdate,
            (
                handle_resize,
                input::handle_global_input,
                input::handle_mouse_input,
                input::handle_input_area_input.run_if(in_state(TuiMainFocus::InputArea)),
            )
                .chain(),
        )
        // Register action execution and rendering systems
        .add_systems(
            Update,
            (
                core::action_system::tui_action_system,
                renderer::draw_scene_system.run_if(resource_exists::<RenderNeeded>),
            ),
        );
}
