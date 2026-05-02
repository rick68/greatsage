mod tui_main;
pub use tui_main::TuiMain;

use {
    bevy::{
        app::{App, PluginGroup, PreUpdate},
        ecs::{change_detection::ResMut, message::MessageReader},
        prelude::{Deref, DerefMut, Resource},
    },
    bevy_ratatui::{
        RatatuiPlugins,
        event::{EventPlugin, ResizeMessage},
    },
};

#[derive(Deref, DerefMut, Resource)]
pub struct RenderNeeded(bool);

impl Default for RenderNeeded {
    fn default() -> Self {
        Self(true)
    }
}

fn handle_resize(mut messages: MessageReader<ResizeMessage>, mut dirty: ResMut<RenderNeeded>) {
    if let Some(ResizeMessage(_size)) = messages.read().next() {
        **dirty = true;
    }
}

pub fn tui_plugin(app: &mut App) {
    app.init_resource::<RenderNeeded>()
        .add_plugins((
            RatatuiPlugins {
                enable_mouse_capture: true,
                enable_input_forwarding: true,
                enable_kitty_protocol: false,
            }
            .set(EventPlugin {
                control_c_interrupt: false,
            }),
            tui_main::plugin,
        ))
        .add_systems(PreUpdate, handle_resize);
}
