use {
    bevy::{
        app::{App, PreUpdate},
        ecs::{change_detection::ResMut, message::MessageReader, system::IsFunctionSystem},
        prelude::{Deref, DerefMut, Resource},
        utils::default,
    },
    bevy_ratatui::{RatatuiPlugins, event::ResizeMessage},
};

#[derive(Deref, DerefMut, Resource)]
pub struct RenderNeeded(bool);

impl Default for RenderNeeded {
    fn default() -> Self {
        Self(true)
    }
}

fn handle_resize(
    mut messages: MessageReader<'_, '_, ResizeMessage>,
    mut dirty: ResMut<'_, RenderNeeded>,
) {
    for ResizeMessage(_size) in messages.read() {
        **dirty = true;
    }
}

pub fn tui_plugin(app: &mut App) {
    let _: &mut App = app
        .add_plugins::<_>(RatatuiPlugins {
            enable_mouse_capture: true,
            enable_input_forwarding: true,
            ..default::<RatatuiPlugins>()
        })
        .init_resource::<RenderNeeded>()
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // MessageReader<'_, '_, ResizeMessage>
                _, // ResMut<'_, RenderNeeded>
            ) -> (),
        )>(PreUpdate, handle_resize);
}
