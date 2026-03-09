#![windows_subsystem = "windows"]

mod coding_agent;
mod tokio;
mod tui;

use {
    crate::{coding_agent::coding_agent_plugin, tokio::tokio_plugin, tui::tui_plugin},
    bevy::{
        MinimalPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin},
        state::app::StatesPlugin,
    },
    std::{path::PathBuf, time::Duration},
};

const FRAMES_PER_SECOND: f32 = 30.0;

fn main() {
    let _: dotenvy::Result<PathBuf> = dotenvy::dotenv();
    let mut app: App = App::new();

    let _: &mut App = app.add_plugins::<(_, _, _, _, _, _)>((
        MinimalPlugins
            .set::<ScheduleRunnerPlugin>(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                FRAMES_PER_SECOND.recip(),
            )))
            .build(),
        StatesPlugin,
        tokio_plugin,
        tui_plugin,
        coding_agent_plugin,
    ));

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
