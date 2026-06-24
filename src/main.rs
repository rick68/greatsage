#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod agents;
mod cli;
mod config;
mod config_paths;
mod env_load;
mod providers;
mod repl;
mod session;
mod setup;
mod stdin;
mod stdout;
mod tokio;
mod utils;

use {
    crate::{
        agents::{CodingAgentPromptChannel, CodingAgentTask, agents_plugin},
        cli::{Cli, Command},
        config::config_plugin,
        repl::repl_plugin,
        session::session_plugin,
        setup::{offer_setup, run_wizard},
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
    env_load::load_layered_env();

    let mut cli = Cli::parse_and_check_help();

    if matches!(cli.command, Some(Command::Setup)) {
        match run_wizard() {
            Ok(()) => return,
            Err(err) => {
                let failed = err.is_failure();
                () = err.report();
                if failed {
                    std::process::exit(1);
                }
                return;
            }
        }
    } else {
        let interactive_repl =
            cli.prompt.is_none() && cli.prompt_file.is_none() && io::stdin().is_terminal();

        if interactive_repl && setup::needs_setup() {
            let run_now = match offer_setup() {
                Ok(run_now) => run_now,
                Err(err) => {
                    let failed = err.is_failure();
                    () = err.report();
                    if failed {
                        std::process::exit(1);
                    }
                    return;
                }
            };

            if run_now {
                match run_wizard() {
                    Ok(()) => env_load::load_layered_env(),
                    Err(err) => {
                        let failed = err.is_failure();
                        () = err.report();
                        if failed {
                            std::process::exit(1);
                        }
                        return;
                    }
                }
            }
        }
    }

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
        let mut buf = String::new();
        if io::stdin().lock().read_to_string(&mut buf).is_ok() {
            let trimmed = buf.trim();
            if !trimmed.is_empty() {
                prompt_arg = Some(trimmed.to_owned());
            }
        }
    }

    let one_shot = prompt_arg
        .as_ref()
        .is_some_and(|prompt| !prompt.trim().is_empty());
    let interactive_repl = !one_shot && io::stdin().is_terminal();

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
        app.add_plugins(session::register_session_brp_methods(
            RemotePlugin::default(),
        ))
        .add_plugins(RemoteHttpPlugin::default().with_port(port));
    }

    if let Some(prompt) = prompt_arg.filter(|prompt| !prompt.trim().is_empty()) {
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
    } else if interactive_repl {
        app.add_plugins((stdin_plugin, repl_plugin));
    } else if cfg!(feature = "dev_native") {
        if !cli.no_hints {
            eprintln!(
                "greatsage: headless dev mode (stdin is not a TTY). \
                 Send prompts via BRP session.send_prompt, or run `cargo run --features dev_native` in a terminal for REPL."
            );
        }
    } else {
        eprintln!("greatsage: stdin is not a terminal; use -p PROMPT or pipe a prompt.");
        std::process::exit(1);
    }

    if let AppExit::Error(code) = app.run() {
        std::process::exit(code.get() as i32);
    }
}
