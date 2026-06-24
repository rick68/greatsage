#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod agents;
mod cli;
mod config;
mod providers;
mod repl;
mod session;
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
        session::session_plugin,
        stdin::stdin_plugin,
        stdout::stdout_plugin,
        tokio::tokio_plugin,
    },
    bevy::{
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
        fs,
        io::{self, IsTerminal, Read},
        time::Duration,
    },
};

const FRAMES_PER_SECOND: f32 = 30.0;

fn schedule_runner() -> ScheduleRunnerPlugin {
    ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(FRAMES_PER_SECOND.recip()))
}

/// `dev_native` pulls render/window plugins via `bevy_brp_extras`; the CLI stays
/// headless and only needs a ticking schedule for ECS + BRP.
fn default_plugins() -> bevy::app::PluginGroupBuilder {
    cfg_if::cfg_if! {
        if #[cfg(feature = "dev_native")] {
            use bevy::{MinimalPlugins, state::app::StatesPlugin};
            MinimalPlugins.set(schedule_runner()).add(StatesPlugin)
        } else {
            use bevy::{MinimalPlugins, state::app::StatesPlugin};
            MinimalPlugins.set(schedule_runner()).add(StatesPlugin)
        }
    }
}

fn main() {
    let _ = dotenvy::dotenv();

    let mut cli = Cli::parse_and_check_help();

    if let Some(file_path) = &cli.prompt_file
        && let Ok(buf) = fs::read_to_string(file_path)
    {
        if let Some((_lhs, rhs)) = buf.trim().trim_start_matches("#!").split_once('\n') {
            let tail = rhs.trim();
            if !tail.is_empty() {
                cli.prompt = Some(tail.to_owned());
            }
        } else {
            return;
        }
    }

    let mut prompt_arg = cli.prompt.clone();

    if !io::stdin().is_terminal() && prompt_arg.is_none() {
        let mut buf: String = String::new();
        let _: usize = io::stdin().lock().read_to_string(&mut buf).unwrap();
        prompt_arg = Some(String::from(buf.trim_end_matches('\n')));
    }

    let mut app = App::new();
    app.insert_resource::<Cli>(cli.clone()).add_plugins((
        default_plugins(),
        tokio_plugin,
        config_plugin,
        stdout_plugin,
        session_plugin,
        agents_plugin,
    ));

    #[cfg(feature = "dev_native")]
    {
        use bevy::remote::{RemotePlugin, http::RemoteHttpPlugin};
        let port = std::env::var("BRP_EXTRAS_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(15702);
        app.add_plugins(RemotePlugin::default())
            .add_plugins(RemoteHttpPlugin::default().with_port(port));
    }

    let prompt_mode = prompt_arg.is_some();
    if !prompt_mode && io::stdin().is_terminal() {
        app.add_plugins(stdin_plugin);
    }

    if let Some(prompt) = prompt_arg {
        app.add_systems(
            Update,
            (
                (move |channel: Res<CodingAgentPromptChannel>| {
                    if let Err(e) = channel.sender.send(prompt.clone()) {
                        eprintln!("Failed to send prompt: {e}");
                    }
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
        std::process::exit(code.get() as i32);
    }
}
