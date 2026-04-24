#![windows_subsystem = "windows"]

mod agents;
mod git;
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
    clap::{ArgAction, Parser, ValueEnum},
    std::{
        env,
        io::{IsTerminal, Read, stdin},
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
    #[arg(long, value_name = "dir", action = ArgAction::Append)]
    skills: Vec<PathBuf>,
    /// MCP server to connect: HTTP URL (e.g. http://localhost:3000) or stdio command (e.g. "npx -y @mcp/server-fs /tmp"). Repeatable.
    #[arg(long, value_name = "server", action = ArgAction::Append)]
    mcp: Vec<String>,
    /// Context management: compaction or checkpoint
    #[arg(long, value_name = "s", default_value = "compaction")]
    context_strategy: ContextStrategy,
    /// Print status messages (MCP connection, ready) to stderr in non-interactive mode
    #[arg(short = 'v', long)]
    verbose: bool,
}

pub fn validate_env_vars() -> Result<(), String> {
    // Collect all missing or empty required environment variables.
    let mut missing = Vec::new();
    for &var in &["BASE_URL", "MODEL", "API_KEY"] {
        // Retrieve the variable; if it exists and is not empty, skip.
        match env::var(var) {
            Ok(val) if !val.trim().is_empty() => continue,
            _ => missing.push(var),
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        // Join missing variables with commas for a clear message.
        Err(format!(
            "Error: Missing required environment variables: {}",
            missing.join(", ")
        ))
    }
}

fn main() {
    _ = dotenvy::dotenv();

    let args = Args::parse();
    // Validate required environment variables after parsing args (so --help works).
    if let Err(msg) = validate_env_vars() {
        eprintln!("{msg}");
        std::process::exit(1);
    }
    let mut invocation_prompt = None;

    {
        let stdin = stdin();
        let Args {
            prompt,
            positional_prompt,
            ..
        } = &args;

        if !stdin.is_terminal() && prompt.is_none() && positional_prompt.is_none() {
            let mut buf = String::new();
            _ = stdin.lock().read_to_string(&mut buf).unwrap();
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
    _ =
        app.insert_resource::<Args>(args)
            .add_plugins(DefaultPlugins.set(ScheduleRunnerPlugin::run_loop(
                Duration::from_secs_f32(FRAMES_PER_SECOND.recip()),
            )));
    // Add other custom plugins
    _ = app.add_plugins((tokio_plugin, agents_plugin));

    if let Some(prompt) = invocation_prompt {
        _ = app.add_systems(
            Update,
            (
                (move |channel: Res<CodingAgentPromptChannel>| {
                    () = channel.sender.send(prompt.clone()).unwrap();
                })
                .run_if(run_once),
                (|mut commands: Commands| {
                    _ = commands.write_message(AppExit::Success);
                })
                .run_if(condition_changed_to(
                    false,
                    resource_exists::<CodingAgentTask>,
                )),
            ),
        );
    } else {
        _ = app.add_plugins(tui_plugin);
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
            env::remove_var("BASE_URL");
            env::remove_var("MODEL");
            env::remove_var("API_KEY");
        }
        let err = validate_env_vars().unwrap_err();
        // The error should list all missing variables.
        assert!(err.contains("BASE_URL"));
        assert!(err.contains("MODEL"));
        assert!(err.contains("API_KEY"));
    }

    #[test]
    #[temp_env_vars]
    fn test_validate_env_missing_partial() {
        // Set only BASE_URL, leave others missing.
        unsafe {
            () = env::set_var("BASE_URL", "https://example.com");
            () = env::remove_var("MODEL");
            () = env::remove_var("API_KEY");
        }
        let err = validate_env_vars().unwrap_err();
        // Should mention only the missing vars.
        assert!(!err.contains("BASE_URL"));
        assert!(err.contains("MODEL"));
        assert!(err.contains("API_KEY"));
    }

    #[test]
    #[temp_env_vars]
    fn test_validate_env_present() {
        // All vars present.
        unsafe {
            () = env::set_var("BASE_URL", "https://example.com");
            () = env::set_var("MODEL", "test-model");
            () = env::set_var("API_KEY", "daummy_key");
        }
        assert!(validate_env_vars().is_ok());
    }
}
