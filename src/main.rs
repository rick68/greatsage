#![windows_subsystem = "windows"]

mod agents;
mod config;
mod evolve;
mod git;
mod tokio;
mod tui;

use {
    crate::{
        agents::{CodingAgentPromptChannel, CodingAgentTask, agents_plugin},
        config::{
            AppConfig, ConfigSubcommand, ContextStrategy, default_config_path,
            run_config_subcommand, validate_required,
        },
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
    clap::{
        ArgAction, CommandFactory, Parser, Subcommand,
        builder::styling::{AnsiColor, Effects, Styles},
    },
    std::{
        env,
        io::{IsTerminal, Read, stdin},
        path::PathBuf,
        process,
        time::Duration,
    },
};

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .invalid(AnsiColor::Yellow.on_default().effects(Effects::BOLD));

#[derive(Clone, Debug, Parser, Resource)]
#[command(version, about, long_about = None, styles = STYLES)]
struct Args {
    /// Path to config file
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    // Model to use (overrides config file)
    #[arg(long, value_name = "name")]
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
    /// MCP server to connect: HTTP URL or stdio command. Repeatable.
    #[arg(long, value_name = "server", action = ArgAction::Append)]
    mcp: Vec<String>,
    /// Context management: compaction or checkpoint (overrides config file)
    #[arg(long, value_name = "s")]
    context_strategy: Option<ContextStrategy>,
    /// Print status messages to stderr in non-interactive mode
    #[arg(short = 'v', long)]
    verbose: bool,
    /// Stage all changes before running the app
    #[arg(long, action = ArgAction::SetTrue)]
    stage_all: bool,
    /// Commit staged changes with the given message after optional staging
    #[arg(long, value_name = "msg")]
    git_commit: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
    /// Run evolve mode (placeholder)
    #[arg(long, action = ArgAction::SetTrue)]
    evolve: bool,
}

#[derive(Subcommand, Clone, Debug)]
enum Command {
    /// View and edit configuration
    Config {
        #[command(subcommand)]
        cmd: ConfigSubcommand,
    },
}

use std::error::Error;

fn handle_prompt(prompt: String) -> Result<(), Box<dyn Error>> {
    // Currently, we simply treat empty prompts as a no-op.
    // Future logic can include more validation.
    if prompt.trim().is_empty() {
        return Ok(());
    }
    // No other side‑effects here; sending is handled elsewhere.
    Ok(())
}

fn main() {
    clap_complete::CompleteEnv::with_factory(Args::command).complete();

    _ = dotenvy::dotenv();

    let args = Args::parse();

    let config_path = args.config.clone().unwrap_or_else(default_config_path);
    let mut app_config = AppConfig::load_or_create(&config_path);

    // Config subcommand: operate on the file and exit.
    if let Some(Command::Config { ref cmd }) = args.command {
        if let Err(e) = run_config_subcommand(cmd, &config_path, &mut app_config) {
            eprintln!("error: {e:#}");
            _ = process::exit(1);
        }
        return;
    }

    // CLI overrides for config-file values.
    if let Some(model) = &args.model {
        app_config.llm.model = model.clone();
    }
    if let Some(strategy) = args.context_strategy {
        app_config.agent.context_strategy = strategy;
    }

    if let Err(e) = validate_required(&app_config) {
        eprintln!("error: {e:#}");
        _ = process::exit(1);
    }

    // Git integration: optional staging and commit.
    if args.stage_all
        && let Err(e) = git::stage_all()
    {
        eprintln!("{}", e);
        _ = process::exit(1);
    }
    if let Some(msg) = &args.git_commit
        && let Err(e) = git::commit(msg)
    {
        eprintln!("{}", e);
        _ = process::exit(1);
    }

    // Evolve mode placeholder
    if args.evolve {
        if let Err(e) = evolve::run_evolve() {
            eprintln!("{}", e);
            _ = process::exit(1);
        }
        _ = process::exit(0);
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

    let frames_per_second = app_config.tui.frames_per_second;
    let mut app: App = App::new();
    _ =
        app.insert_resource(app_config)
            .insert_resource::<Args>(args)
            .add_plugins(DefaultPlugins.set(ScheduleRunnerPlugin::run_loop(
                Duration::from_secs_f32(frames_per_second.recip()),
            )));
    _ = app.add_plugins((tokio_plugin, agents_plugin));

    if let Some(prompt) = invocation_prompt {
        // Use helper to handle the prompt with error handling.
        let prompt_clone = prompt.clone();
        _ = app.add_systems(
            Update,
            (
                (move |channel: Res<CodingAgentPromptChannel>| {
                    // First, handle prompt validation.
                    if let Err(e) = handle_prompt(prompt_clone.clone()) {
                        eprintln!("Prompt handling error: {e}");
                        return;
                    }
                    // Wrap send in panic catcher and forward errors.
                    let result =
                        std::panic::catch_unwind(|| channel.sender.send(prompt_clone.clone()));
                    match result {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            eprintln!("Error sending prompt: {e:?}");
                        }
                        Err(panic) => {
                            eprintln!("Panic while sending prompt: {panic:?}");
                        }
                    }
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
        () = process::exit(code.get() as i32);
    }
}

/// Validate required env vars (pure env check, used in tests).
pub fn validate_env_vars() -> Result<(), String> {
    let mut missing = Vec::new();
    for &var in &["BASE_URL", "MODEL", "API_KEY"] {
        match env::var(var) {
            Ok(val) if !val.trim().is_empty() => continue,
            _ => missing.push(var),
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Error: Missing required environment variables: {}",
            missing.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use {super::*, temp_env_vars::temp_env_vars};

    #[test]
    #[temp_env_vars]
    fn test_validate_env_missing_all() {
        static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = TEST_MUTEX.lock().unwrap();
        unsafe {
            env::remove_var("BASE_URL");
            env::remove_var("MODEL");
            env::remove_var("API_KEY");
        }
        let err = validate_env_vars().unwrap_err();
        assert!(err.contains("BASE_URL"));
        assert!(err.contains("MODEL"));
        assert!(err.contains("API_KEY"));
    }

    #[test]
    #[temp_env_vars]
    fn test_validate_env_missing_partial() {
        unsafe {
            () = std::env::set_var("BASE_URL", "https://example.com");
            () = std::env::remove_var("MODEL");
            () = std::env::remove_var("API_KEY");
        }
        let err = validate_env_vars().unwrap_err();
        assert!(!err.contains("BASE_URL"));
        assert!(err.contains("MODEL"));
        assert!(err.contains("API_KEY"));
    }

    #[test]
    #[temp_env_vars]
    fn test_validate_env_present() {
        unsafe {
            () = std::env::set_var("BASE_URL", "https://example.com");
            () = std::env::set_var("MODEL", "test-model");
            () = std::env::set_var("API_KEY", "dummy_key");
        }
        assert!(validate_env_vars().is_ok());
    }

    #[test]
    fn test_args_parsing_stage_and_commit() {
        let args = Args::parse_from(["test_bin", "--stage-all", "--git-commit", "Initial commit"]);
        assert!(args.stage_all);
        assert_eq!(args.git_commit.as_deref(), Some("Initial commit"));
    }

    #[test]
    fn test_args_parsing_evolve_flag() {
        let args = Args::parse_from(["test_bin", "--evolve"]);
        assert!(args.evolve);
    }

    #[test]
    fn test_handle_prompt_empty() {
        // Empty prompt should be handled without error.
        assert!(handle_prompt("".to_string()).is_ok());
    }
}
