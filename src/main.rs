#![windows_subsystem = "windows"]

mod plugins;

use {
    crate::plugins::{tokio_plugin, tui_plugin},
    bevy::{
        MinimalPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin},
    },
    std::time::Duration,
};

const FRAMES_PER_SECOND: f32 = 30.0;

fn main() {
    let mut app: App = App::new();

    let _: &mut App = app.add_plugins::<(_, _, _, _)>((
        MinimalPlugins
            .set::<ScheduleRunnerPlugin>(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                FRAMES_PER_SECOND.recip(),
            )))
            .build(),
        tokio_plugin,
        tui_plugin,
    ));

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
