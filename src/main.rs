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
        env,
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
    /// Positional prompt argument (alternative to --prompt)
    #[arg(value_name = "prompt", required = false)]
    positional_prompt: Option<String>,
    /// Directory containing skill files
    #[arg(long, value_name = "dir")]
    skills: Option<Vec<PathBuf>>,
    /// Context management: compaction or checkpoint
    #[arg(long, value_name = "s", default_value = "compaction")]
    context_strategy: ContextStrategy,
}

pub fn validate_env_vars() -> Result<(), String> {
    // Collect all missing or empty required environment variables.
    let mut missing: Vec<&str> = Vec::new();
    for &var in &["BASE_URL", "MODEL", "API_KEY"] {
        // Retrieve the variable; if it exists and is not empty, skip.
        match env::var(var) {
            Ok(val) if !val.trim().is_empty() => continue,
            _ => () = missing.push(var),
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        // Join missing variables with commas for a clear message.
        Err(format!(
            "Error: Missing required environment variables: {}",
            missing.join::<&str>(", ")
        ))
    }
}

fn main() {
    let _: dotenvy::Result<PathBuf> = dotenvy::dotenv();

    let args: Args = Args::parse();
    // Validate required environment variables after parsing args (so --help works).
    if let Err(msg) = validate_env_vars() {
        eprintln!("{msg}");
        std::process::exit(1);
    }
    let mut invocation_prompt: Option<String> = None;

    {
        let stdin: Stdin = stdin();
        let Args {
            prompt,
            positional_prompt,
            ..
        } = &args;

        if !stdin.is_terminal() && prompt.is_none() && positional_prompt.is_none() {
            let mut buf: String = String::new();
            let _: usize = stdin.lock().read_to_string(&mut buf).unwrap();
            invocation_prompt = Some(buf);
        } else if prompt.is_some() || positional_prompt.is_some() {
            invocation_prompt = match (prompt, positional_prompt) {
                (Some(p), None) => Some(p.clone()),
                (None, Some(p)) => Some(p.clone()),
                (Some(p1), Some(p2)) => Some(format!("{p1}{p2}")),
                _ => None,
            };
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

    if let Some(prompt) = invocation_prompt {
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

#[cfg(test)]
mod tests {
    use {super::*, temp_env_vars::temp_env_vars};

    #[test]
    #[temp_env_vars]
    fn test_validate_env_missing_all() {
        // Ensure all required vars are absent.
        static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = TEST_MUTEX.lock().unwrap();
        unsafe {
            env::remove_var("API_KEY");
            env::remove_var("BASE_URL");
            env::remove_var("MODEL");
        }
        let err: String = validate_env_vars().unwrap_err();
        // The error should list all missing variables.
        assert!(err.contains("API_KEY"));
        assert!(err.contains("BASE_URL"));
        assert!(err.contains("MODEL"));
    }

    #[test]
    #[temp_env_vars]
    fn test_validate_env_missing_partial() {
        // Set only BASE_URL, leave others missing.
        unsafe {
            () = env::remove_var::<&str>("API_KEY");
            () = env::set_var::<&str, &str>("BASE_URL", "https://example.com");
            () = env::remove_var::<&str>("MODEL");
        }
        let err: String = validate_env_vars().unwrap_err();
        // Should mention only the missing vars.
        assert!(err.contains("API_KEY"));
        assert!(!err.contains("BASE_URL"));
        assert!(err.contains("MODEL"));
    }

    #[test]
    #[temp_env_vars]
    fn test_validate_env_present() {
        // All vars present.
        unsafe {
            () = env::set_var::<&str, &str>("API_KEY", "daummy_key");
            () = env::set_var::<&str, &str>("BASE_URL", "https://example.com");
            () = env::set_var::<&str, &str>("MODEL", "test-model");
        }
        assert!(validate_env_vars().is_ok());
    }
}
