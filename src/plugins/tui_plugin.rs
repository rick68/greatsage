use {
    bevy::{app::App, utils::default},
    bevy_ratatui::RatatuiPlugins,
};

pub fn plugin(app: &mut App) {
    let _: &mut App = app.add_plugins::<_>(RatatuiPlugins {
        enable_mouse_capture: true,
        enable_input_forwarding: true,
        ..default::<RatatuiPlugins>()
    });
}
