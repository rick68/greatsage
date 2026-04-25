#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

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

use std::error::Error;

build_info::build_info!(fn build_info);

pub fn handle_prompt(prompt: String, error_handling: bool) -> Result<(), Box<dyn Error>> {
    // Treat empty prompts as a no-op.
    if prompt.trim().is_empty() {
        return Ok(());
    }

    // If error handling flag is enabled, perform validation.
    if error_handling {
        // Determine if the prompt looks like a file reference.
        // Recognize "file:" prefix or known file extensions.
        let trimmed = prompt.trim();
        let path_candidate = if let Some(stripped) = trimmed.strip_prefix("file:") {
            stripped.trim()
        } else {
            trimmed
        };
        const KNOWN_EXTS: &[&str] = &[".rs", ".txt", ".md", ".json", ".jsonl", ".toml"]; // extend as needed
        let looks_like_file = KNOWN_EXTS.iter().any(|ext| path_candidate.ends_with(ext));
        if looks_like_file {
            // Verify file exists and is readable.
            match std::fs::metadata(path_candidate) {
                Ok(meta) if meta.is_file() => {
                    // try opening for reading to ensure readability
                    std::fs::File::open(path_candidate)?;
                }
                _ => {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("File not found or unreadable: {path_candidate}"),
                    )));
                }
            }
        }
    }

    // No further side‑effects here; sending is handled elsewhere.
    Ok(())
}

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
    // Stats subcommand: display assessment information.
    if let Some(Command::Stats) = args.command {
        match evolve::assessment_phase(std::path::Path::new(".")) {
            Ok(info) => {
                println!("{info}");
                _ = process::exit(0);
            }
            Err(e) => {
                eprintln!("error: {e}");
                _ = process::exit(1);
            }
        }
    }

    // CLI overrides for config-file values.
    if let Some(model) = &args.model {
        app_config.llm.model = model.clone();
    }
    if let Some(strategy) = args.context_strategy {
        app_config.agent.context_strategy = strategy;
    }
    if let Some(thinking) = args.thinking {
        app_config.llm.thinking_level = thinking;
    }
    if let Some(max_tokens) = args.max_tokens {
        app_config.llm.max_tokens = max_tokens;
    }
    if let Some(max_turns) = args.max_turns {
        app_config.agent.max_turns = max_turns;
    }
    if let Some(temperature) = args.temperature {
        app_config.llm.temperature = Some(temperature);
    }

    // Populate runtime-only fields: config file first, CLI appended after (CLI wins on conflict).
    app_config.runtime.skills = args.skills.clone();
    app_config.runtime.mcp_servers = app_config
        .mcp
        .sse_transports
        .iter()
        .chain(app_config.mcp.stdio_transports.iter())
        .cloned()
        .chain(args.mcp.iter().cloned())
        .collect();
    app_config.runtime.verbose = args.verbose;
    app_config.runtime.error_handling = args.error_handling;
    // Capture error handling flag before moving app_config into Bevy resource.
    let error_handling_flag = app_config.runtime.error_handling;

    if let Err(e) = validate_required(&app_config) {
        eprintln!("error: {e:#}");
        _ = process::exit(1);
    }

    // Evolve mode placeholder
    if args.evolve {
        if let Err(e) = evolve::run_evolve() {
            eprintln!("{e}");
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
        // Use helper to handle the prompt with error handling.
        let prompt_clone = prompt.clone();
        let error_handling = error_handling_flag;
        _ = app.add_systems(
            Update,
            (
                (move |channel: Res<CodingAgentPromptChannel>| {
                    // First, handle prompt validation.
                    if let Err(e) = handle_prompt(prompt_clone.clone(), error_handling) {
                        eprintln!("Prompt handling error: {e}");
                        return;
                    }
                    // Wrap send in panic catcher and forward errors.
                    let result =
                        std::panic::catch_unwind(|| channel.sender.send(prompt_clone.clone()));
                    match result {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => eprintln!("Error sending prompt: {e:?}"),
                        Err(panic) => eprintln!("Panic while sending prompt: {panic:?}"),
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
    mod cli_stats;
    mod repl_error_handling;
    mod truncate;

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
    fn test_args_parsing_evolve_flag() {
        let args = Args::parse_from(["test_bin", "--evolve"]);
        assert!(args.evolve);
    }
}
