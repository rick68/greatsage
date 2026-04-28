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
        env, fs,
        io::{self, IsTerminal, Read, Write},
        path::Path,
        process,
        time::Duration,
    },
};

use std::error::Error;

// Alias for error type used throughout REPL handling.
// Allows returning any error that implements the `Error` trait.
// This keeps the public API simple while supporting diverse error sources.
pub type ReplError = Box<dyn Error>;

/// Persist the REPL error handling flag if it changed.
fn maybe_save_repl_error_handling(app_config: &mut AppConfig, config_path: &Path, original: bool) {
    if app_config.repl_error_handling != original
        && let Err(e) = app_config.save(config_path)
    {
        eprintln!("Failed to save config: {e}");
    }
}

/// Install a panic hook that aborts with a clear message and exit code 101
/// when `--strict-errors` is enabled. This provides a guard‑rail so that REPL
/// panics do not silently crash the process.
fn maybe_set_strict_error_hook(enabled: bool) {
    if enabled {
        std::panic::set_hook(Box::new(|panic_info| {
            let _ = writeln!(std::io::stderr(), "panic: {}", panic_info);
            std::process::exit(101);
        }));
    }
}

build_info::build_info!(fn build_info);

pub fn handle_prompt(prompt: String, repl_error_handling: bool) -> Result<(), ReplError> {
    let trimmed = prompt.trim();

    // Treat empty prompts as a no-op.
    if trimmed.is_empty() {
        return Ok(());
    }

    // If error handling flag is enabled, perform validation.
    if repl_error_handling {
        // Determine if the prompt looks like a file reference.
        // Recognize "file:" prefix or known file extensions.
        let path_candidate = if let Some(stripped) = trimmed.strip_prefix("file:") {
            stripped.trim()
        } else {
            trimmed
        };
        const KNOWN_EXTS: &[&str] = &[".rs", ".txt", ".md", ".json", ".jsonl", ".toml"]; // extend as needed
        let looks_like_file = KNOWN_EXTS.iter().any(|ext| path_candidate.ends_with(ext));
        if looks_like_file {
            // Verify file exists and is readable.
            match fs::metadata(path_candidate) {
                Ok(meta) if meta.is_file() => {
                    // try opening for reading to ensure readability
                    _ = fs::File::open(path_candidate)?;
                }
                _ => {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("File not found or unreadable: {path_candidate}"),
                    )));
                }
            }
        }
    }

    // No further side‑effects here; sending is handled elsewhere.
    // Simulate forced panic for testing if environment variable is set.
    if repl_error_handling && std::env::var("FORCE_PANIC").as_deref() == Ok("1") {
        panic!("Forced panic for REPL error handling test");
    }
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
    app_config.runtime.strict_errors = args.strict_errors;
    app_config.runtime.error_handling = args.error_handling;
    // REPL error handling: CLI flag overrides persisted config
    // Determine REPL error handling flag: --check overrides others, then --error-handling, then persisted config.
    // Determine REPL error handling: --check overrides others, then --error-handling, then persisted config.
    // Capture original value before potentially updating.
    let original_repl_error_handling = app_config.repl_error_handling;
    app_config.repl_error_handling =
        args.check || args.error_handling || args.handle_errors || args.repl_error_handling;
    // Persist the REPL error handling flag if it changed.
    maybe_save_repl_error_handling(&mut app_config, &config_path, original_repl_error_handling);
    // Capture REPL error handling flag before moving app_config into Bevy resource.
    let repl_error_handling_flag = app_config.repl_error_handling;
    // Install panic hook for strict error handling if enabled.
    () = maybe_set_strict_error_hook(app_config.runtime.strict_errors);

    if let Err(e) = validate_required(&app_config) {
        eprintln!("error: {e:#}");
        _ = process::exit(1);
    }

    // Evolve subcommand (supports dry-run)
    if let Some(Command::Evolve { dry_run, push }) = args.command {
        if dry_run {
            if let Err(e) = evolve::run_evolve_dry() {
                eprintln!("{e}");
                _ = process::exit(1);
            }
        } else {
            // Run evolve pipeline
            if let Err(e) = evolve::run_evolve() {
                eprintln!("{e}");
                _ = process::exit(1);
            }
            // After successful evolve, commit and tag
            // Read iteration count
            let iteration_res = std::fs::read_to_string("ITERATION_COUNT");
            let iteration: u32 = match iteration_res {
                Ok(s) => s.trim().parse().unwrap_or(0),
                Err(_) => 0,
            };
            if let Err(e) = crate::git::commit_and_tag(iteration, push) {
                eprintln!("Git error: {e}");
                _ = process::exit(1);
            }
        }
        _ = process::exit(0);
    }
    // Backward‑compatible --evolve flag (legacy). Executes the evolve pipeline.
    if args.evolve {
        // Historically this flag was deprecated; we now treat it as an alias for the
        // `evolve` subcommand without requiring the `dry_run` or `push` options.
        // It runs the full evolution pipeline and then exits.
        if let Err(e) = evolve::run_evolve() {
            eprintln!("{e}");
            _ = process::exit(1);
        }
        _ = process::exit(0);
    }

    let mut invocation_prompt = None;
    {
        let stdin = io::stdin();
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
        let repl_error_handling = repl_error_handling_flag;
        _ = app.add_systems(
            Update,
            (
                (move |channel: Res<CodingAgentPromptChannel>| {
                    // First, handle prompt validation.
                    if let Err(e) = handle_prompt(prompt_clone.clone(), repl_error_handling) {
                        eprintln!("Prompt handling error: {e}");
                        return;
                    }
                    // Conditionally wrap send in panic catcher based on REPL error handling flag.
                    if repl_error_handling {
                        // Wrap send in panic catcher and forward errors.
                        let result =
                            std::panic::catch_unwind(|| channel.sender.send(prompt_clone.clone()));
                        match result {
                            Ok(Ok(())) => {}
                            Ok(Err(e)) => eprintln!("Error sending prompt: {e:?}"),
                            Err(panic) => eprintln!("Panic while sending prompt: {panic:?}"),
                        }
                    } else {
                        // Direct send without panic catching.
                        if let Err(e) = channel.sender.send(prompt_clone.clone()) {
                            eprintln!("Error sending prompt: {e:?}");
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

    // Run the app, optionally catching panics for global error handling.
    let run_result = if repl_error_handling_flag {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.run()))
    } else {
        Ok(app.run())
    };
    match run_result {
        Ok(AppExit::Error(code)) => {
            () = process::exit(code.get() as i32);
        }
        Ok(_) => {}
        Err(panic) => {
            eprintln!("Error: REPL encountered an unexpected panic: {panic:?}");
            std::process::exit(1);
        }
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
    mod check_flag;
    mod cli_stats;
    mod evolve_protection;
    mod repl_error_handling;
    mod task_01_execution;
    mod task_01_placeholder;
    mod task_02_execution;
    mod task_02_placeholder;
    mod task_03_execution;
    mod task_03_placeholder;
    mod task_40_execution;
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
