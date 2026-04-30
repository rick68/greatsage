#![windows_subsystem = "windows"]

mod agents;
mod tokio;
mod tui;

use {
    crate::{
        agents::{CodingAgentPromptChannel, CodingAgentTask, agents_plugin},
        tokio::tokio_plugin,
        tui::tui_plugin,
    },
    bevy::{
        DefaultPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin, Update},
        ecs::{
            change_detection::Res,
            resource::Resource,
            schedule::{
                IntoScheduleConfigs,
                common_conditions::{condition_changed_to, resource_exists, run_once},
            },
            system::Commands,
        },
    },
    clap::{Parser, ValueEnum},
    std::{
        io::{IsTerminal, Read, Stdin, stdin},
        path::PathBuf,
        time::Duration,
    },
};

const FRAMES_PER_SECOND: f32 = 30.0;

/// Context management strategy.
#[derive(Clone, Copy, Debug, Default, PartialEq, ValueEnum)]
pub enum ContextStrategy {
    /// Default: auto-compact conversation when approaching context limit
    #[default]
    Compaction,
    /// Write checkpoint file and exit with code 2 when approaching limit
    Checkpoint,
}

#[derive(Clone, Debug, Parser, Resource)]
#[command(version, about, long_about = None)]
struct Args {
    // Model to use
    #[arg(long, value_name = "name", default_value = "claude-opus-4-7")]
    model: Option<String>,
    /// Run a single prompt and exit (no REPL)
    #[arg(short, long, value_name = "t")]
    prompt: Option<String>,
    /// Directory containing skill files
    #[arg(long, value_name = "dir")]
    skills: Option<Vec<PathBuf>>,
    /// Context management: compaction or checkpoint
    #[arg(long, value_name = "s", default_value = "compaction")]
    context_strategy: ContextStrategy,
}

fn main() {
    let _ = dotenvy::dotenv();

    let args = Args::parse();
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
    app.insert_resource::<Args>(args);
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
