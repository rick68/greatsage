#![windows_subsystem = "windows"]

mod agents;
mod cli;
mod config;
mod providers;
mod tokio;
mod tui;
pub mod utils;

use {
    crate::{
        agents::{AgentConfig, CodingAgentPromptChannel, CodingAgentTask, agents_plugin},
        cli::{Cli, Command},
        config::Config,
        providers::Provider,
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
        utils::default,
    },
    clap::{CommandFactory, Parser},
    clap_help::Printer,
    std::{
        io::{IsTerminal, Read, stdin},
        time::Duration,
    },
    termimad::crossterm::style::Color::{DarkYellow, White},
};

const FRAMES_PER_SECOND: f32 = 30.0;

fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();
    // Load configuration file (default to $HOME/.greatsage.toml if not provided)
    let config_path = cli.config.clone().or_else(|| {
        std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join(".greatsage.toml"))
    });
    let config = match config_path {
        Some(ref p) => Config::load(p)
            .map_err(|e| anyhow::anyhow!("Failed to load config file {}: {e}", p.display()))?,
        None => Config::default(),
    };
    // Capture provider flag into AgentConfig for agents to use (override config if CLI flag set)
    let agent_config = AgentConfig {
        provider: cli
            .provider
            .or(config.provider)
            .or(Some(Provider::Anthropic)),
        ..default()
    };
    // Handle explicit help subcommand
    if cli.help || Some(Command::Help) == cli.command {
        // Print the help/usage information and exit gracefully.
        let intro = Cli::command().get_about().unwrap_or_default().to_string();
        let mut printer = Printer::new(Cli::command()).with("introduction", intro.as_str());
        let skin = printer.skin_mut();

        skin.headers[0].compound_style.set_fg(DarkYellow);
        skin.inline_code.set_fg(White);
        printer.print_help();

        return Ok(());
    }
    let mut prompt_arg = cli.prompt.clone();

    if !stdin().is_terminal() && prompt_arg.is_none() {
        let mut buf = String::new();
        stdin().lock().read_to_string(&mut buf)?;
        prompt_arg = Some(buf);
    }

    let mut app = App::new();
    app.insert_resource::<Cli>(cli.clone());
    // Insert AgentConfig with provider flag for agents
    app.insert_resource::<AgentConfig>(agent_config);
    app.add_plugins((
        DefaultPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
            FRAMES_PER_SECOND.recip(),
        ))),
        tokio_plugin,
        agents_plugin,
    ));

    if let Some(prompt) = prompt_arg {
        // One‑shot prompt mode: send the prompt then exit.
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
        app.add_plugins(tui_plugin);
    }

    let exit = app.run();
    if let AppExit::Error(code) = exit {
        std::process::exit(code.get() as i32);
    }
    Ok(())
}
