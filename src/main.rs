#![windows_subsystem = "windows"]

mod agents;
mod cli;
mod config;
mod evolve;
mod git;
mod tokio;
mod tui;

use {
    crate::{
        agents::{CodingAgentPromptChannel, CodingAgentTask, agents_plugin},
        cli::{Args, Command, complete},
        config::{AppConfig, run_config_subcommand, validate_required},
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
        env,
        io::{IsTerminal, Read, stdin},
        process,
        time::Duration,
    },
};

fn main() {
    () = complete();

    _ = dotenvy::dotenv();

    let args = Args::parse();

    let config_path = args.config.clone();
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

    // Populate runtime-only fields from CLI flags.
    app_config.runtime.skills = args.skills.clone();
    app_config.runtime.mcp_servers = args.mcp.clone();
    app_config.runtime.verbose = args.verbose;

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
            .add_plugins(DefaultPlugins.set(ScheduleRunnerPlugin::run_loop(
                Duration::from_secs_f32(frames_per_second.recip()),
            )));
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
            () = env::set_var("BASE_URL", "https://example.com");
            () = env::remove_var("MODEL");
            () = env::remove_var("API_KEY");
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
            () = env::set_var("BASE_URL", "https://example.com");
            () = env::set_var("MODEL", "test-model");
            () = env::set_var("API_KEY", "dummy_key");
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
    fn test_run_evolve_placeholder() {
        // Ensure run_evolve returns Ok without panic.
        () = evolve::run_evolve().expect("run_evolve should succeed");
    }
}
