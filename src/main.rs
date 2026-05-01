#![windows_subsystem = "windows"]

mod agents;
mod cli;
mod tokio;
mod tui;
mod utils;

use {
    crate::{
        agents::{CodingAgentPromptChannel, CodingAgentTask, agents_plugin},
        cli::Cli,
        tokio::tokio_plugin,
        tui::tui_plugin,
    },
    bevy::{
        DefaultPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin, Update},
        ecs::{
            change_detection::Res,
            schedule::{
                IntoScheduleConfigs,
                common_conditions::{condition_changed_to, resource_exists, run_once},
            },
            system::Commands,
        },
    },
    clap::Parser,
    std::{
        io::{IsTerminal, Read, Stdin, stdin},
        time::Duration,
    },
};

const FRAMES_PER_SECOND: f32 = 30.0;

fn main() {
    let _ = dotenvy::dotenv();

    let args = Cli::parse();
    let mut prompt_arg = args.prompt.clone();

    {
        let stdin: Stdin = stdin();

        if !stdin.is_terminal() && prompt_arg.is_none() {
            let mut buf: String = String::new();
            let _: usize = stdin.lock().read_to_string(&mut buf).unwrap();
            prompt_arg = Some(buf);
        }
    }

    let mut app = App::new();
    app.insert_resource::<Cli>(args);
    app.add_plugins((
        DefaultPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
            FRAMES_PER_SECOND.recip(),
        ))),
        tokio_plugin,
        agents_plugin,
    ));

    if let Some(prompt) = prompt_arg {
        app.add_systems(
            Update,
            (
                (move |channel: Res<CodingAgentPromptChannel>| {
                    channel.sender.send(prompt.clone()).unwrap();
                })
                .run_if(run_once),
                (|mut commands: Commands| {
                    commands.write_message::<AppExit>(AppExit::Success);
                })
                .run_if(condition_changed_to(
                    false,
                    resource_exists::<CodingAgentTask>,
                )),
            ),
        );
    } else {
        app.add_plugins(tui_plugin);
    }

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
