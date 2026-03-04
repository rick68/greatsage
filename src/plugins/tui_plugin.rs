use {
    bevy::{
        app::App,
        prelude::{Deref, DerefMut, Resource},
        utils::default,
    },
    bevy_ratatui::RatatuiPlugins,
};

#[derive(Deref, DerefMut, Resource)]
pub struct RenderNeeded(bool);

impl Default for RenderNeeded {
    fn default() -> Self {
        Self(true)
    }
}

pub fn plugin(app: &mut App) {
    let _: &mut App = app
        .add_plugins::<_>(RatatuiPlugins {
            enable_mouse_capture: true,
            enable_input_forwarding: true,
            ..default::<RatatuiPlugins>()
        })
        .init_resource::<RenderNeeded>();
}
