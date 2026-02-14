#![windows_subsystem = "windows"]

use {
    bevy::{
        MinimalPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin},
        utils::default,
    },
    bevy_ratatui::RatatuiPlugins,
    std::time::Duration,
};

mod tokio_plugin;

const FRAMES_PER_SECOND: f32 = 30.0;

fn main() {
    let mut app: App = App::new();

    let _: &mut App = app.add_plugins::<(_, _, _, _)>((
        MinimalPlugins
            .set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                FRAMES_PER_SECOND.recip(),
            )))
            .build(),
        tokio_plugin::plugin,
        RatatuiPlugins {
            enable_input_forwarding: true,
            ..default::<RatatuiPlugins>()
        },
    ));

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
