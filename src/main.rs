#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod agents;
mod cli;
mod config;
mod repl;
mod stdin;
mod stdout;
mod tokio;
mod utils;

use {
    crate::{
        agents::{CodingAgentPromptChannel, CodingAgentTask, agents_plugin},
        cli::Cli,
        config::config_plugin,
        repl::repl_plugin,
        stdin::stdin_plugin,
        stdout::stdout_plugin,
        tokio::tokio_plugin,
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
    std::{
        io::{self, IsTerminal, Read, Write},
        time::Duration,
    },
};

const FRAMES_PER_SECOND: f32 = 30.0;

fn main() {
    let _ = dotenvy::dotenv();

    let cli = Cli::parse_and_check_help();
    let mut prompt_arg = cli.prompt.clone();

    if !io::stdin().is_terminal() && prompt_arg.is_none() {
        let mut buf: String = String::new();
        let _: usize = io::stdin().lock().read_to_string(&mut buf).unwrap();
        prompt_arg = Some(buf.trim_end_matches('\n').to_string());
    }

    let mut app = App::new();
    app.insert_resource::<Cli>(cli.clone()).add_plugins((
        DefaultPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
            FRAMES_PER_SECOND.recip(),
        ))),
        tokio_plugin,
        config_plugin,
        stdin_plugin,
        stdout_plugin,
        agents_plugin,
    ));

    if let Some(prompt) = prompt_arg {
        app.add_systems(
            Update,
            (
                (move |channel: Res<CodingAgentPromptChannel>| {
                    if let Err(e) = channel.sender.send(prompt.clone()) {
                        eprintln!("Failed to send prompt: {e}");
                    }
                    let mut lock = io::stdout().lock();
                    let _ = lock.write(format!("{prompt}\r\n").as_bytes());
                    let _ = lock.flush();
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
        app.add_plugins(repl_plugin);
    }

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
