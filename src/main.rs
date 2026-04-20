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
                IntoScheduleConfigs, ScheduleConfigTupleMarker,
                common_conditions::{condition_changed_to, resource_exists, run_once},
            },
            system::{Commands, IsFunctionSystem},
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
    #[arg(long, value_name = "t")]
    prompt: Option<String>,
    /// Directory containing skill files
    #[arg(long, value_name = "dir")]
    skills: Option<Vec<PathBuf>>,
    /// Context management: compaction or checkpoint
    #[arg(long, value_name = "s", default_value = "compaction")]
    context_strategy: ContextStrategy,
}

fn main() {
    let _: dotenvy::Result<PathBuf> = dotenvy::dotenv();

    let args: Args = Args::parse();
    let mut prompt_arg: Option<String> = args.prompt.clone();

    {
        let stdin: Stdin = stdin();

        if !stdin.is_terminal() && prompt_arg.is_none() {
            let mut buf: String = String::new();
            let _: usize = stdin.lock().read_to_string(&mut buf).unwrap();
            prompt_arg = Some(buf);
        }
    }

    let mut app: App = App::new();
    let _: &mut App = app.insert_resource::<Args>(args);
    let _: &mut App = app.add_plugins::<(_, _, _, _)>((
        DefaultPlugins.set::<ScheduleRunnerPlugin>(ScheduleRunnerPlugin::run_loop(
            Duration::from_secs_f32(FRAMES_PER_SECOND.recip()),
        )),
        tokio_plugin,
        agents_plugin,
    ));

    if let Some(prompt) = prompt_arg {
        let _: &mut App = app.add_systems::<(ScheduleConfigTupleMarker, (), ())>(
            Update,
            (
                (move |channel: Res<'_, CodingAgentPromptChannel>| {
                    () = channel.sender.send(prompt.clone()).unwrap();
                })
                .run_if::<(
                    IsFunctionSystem,
                    fn(
                        _, // Local<'_, bool>
                    ) -> bool,
                )>(run_once),
                (|mut commands: Commands<'_, '_>| {
                    let _: &mut Commands<'_, '_> =
                        commands.write_message::<AppExit>(AppExit::Success);
                })
                .run_if::<()>(condition_changed_to::<
                    (
                        IsFunctionSystem,
                        fn(
                            Option<
                                _, // Res<'_, CodingAgentTask>
                            >,
                        ) -> bool,
                    ),
                    (),
                    fn(Option<Res<'_, CodingAgentTask>>) -> bool,
                >(
                    false, resource_exists::<CodingAgentTask>
                )),
            ),
        );
    } else {
        let _: &mut App = app.add_plugins::<_>(tui_plugin);
    }

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
