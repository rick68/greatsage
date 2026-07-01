//! Interactive setup wizard (`greatsage setup` and first-run offer).
//!
//! Prompt flow and copy follow [yoyo-evolve `setup.rs`](https://github.com/yologdev/yoyo-evolve):
//! step headers, dim hint lines, bracketed defaults via `inquire::Text::with_default`.
//!
//! Press **Esc** to go back one step (nested prompts unwind first). At the first step, Esc
//! cancels the wizard. **Ctrl+C** still cancels the entire wizard immediately.

use {
    super::{
        config_io::{
            ExistingSetup, SaveLocation, SaveResult, WizardConfig, default_model_for_provider,
            known_models_for_provider, load_existing_setup, write_config,
        },
        detect::provider_env_var,
    },
    crate::providers::{PROVIDER_SPECS, Provider},
    colored::Colorize,
    inquire::{Confirm, InquireError, Select, Text},
    std::{
        env,
        io::{self, IsTerminal},
    },
    thiserror::Error,
};

// ── Public types ────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("setup cancelled")]
    Cancelled,
    #[error("failed to write config: {0}")]
    Io(#[from] io::Error),
    #[error("setup requires an interactive terminal")]
    NotInteractive,
    #[error("setup prompt failed: {0}")]
    Prompt(String),
}

impl From<InquireError> for SetupError {
    fn from(err: InquireError) -> Self {
        match err {
            InquireError::OperationCanceled | InquireError::OperationInterrupted => Self::Cancelled,
            other => Self::Prompt(other.to_string()),
        }
    }
}

impl SetupError {
    pub fn is_failure(&self) -> bool {
        !matches!(self, SetupError::Cancelled)
    }

    pub fn report(self) {
        match self {
            SetupError::Cancelled => eprintln!("\nSetup cancelled."),
            SetupError::NotInteractive => {}
            SetupError::Io(err) => eprintln!("Setup failed: {err}"),
            SetupError::Prompt(message) => eprintln!("Setup error: {message}"),
        }
    }
}

const CUSTOM_MODEL_LABEL: &str = "Enter a custom model...";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WizardStep {
    Provider,
    ApiKeyConfirm,
    ApiKey,
    Model,
    BaseUrl,
    Save,
}

// ── Display helpers (yoyo-style) ────────────────────────────────────────────

fn print_welcome() {
    println!();
    println!("  {}", "Welcome to greatsage!".bold());
    println!(
        "  {}",
        "Let's get you set up. This will only take a moment.".dimmed()
    );
}

fn print_step(step: u8, title: &str) {
    println!();
    println!("  {}: {title}", format!("Step {step}").bold());
}

fn print_hint(hint: &str) {
    println!("  {}", hint.dimmed());
}

fn print_navigation_hint(cancellable: bool) {
    if cancellable {
        () = print_hint("Press Esc to cancel setup");
    } else {
        () = print_hint("Press Esc to go back");
    }
}

fn print_bullet(line: &str) {
    println!("    {}", line.dimmed());
}

fn print_ok(message: &str) {
    println!("  {} {message}", "✓".green());
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn apply_api_key_to_env(provider: Provider, api_key: Option<&str>) {
    let Some(key) = api_key.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    if let Some(var) = provider_env_var(provider) {
        // SAFETY: called before App::new(); no other threads read env yet.
        unsafe { env::set_var(var, key) };
    } else if provider == Provider::Custom {
        unsafe { env::set_var("API_KEY", key) };
    }
}

fn provider_from_label(label: &str) -> Provider {
    PROVIDER_SPECS
        .iter()
        .find(|spec| spec.wizard_label == label)
        .expect("label comes from PROVIDER_SPECS choices")
        .provider
}

fn provider_label(provider: Provider) -> &'static str {
    provider.wizard_label()
}

fn env_api_key(provider: Provider) -> Option<String> {
    provider_env_var(provider)
        .and_then(|var| env::var(var).ok())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            if provider == Provider::Custom {
                env::var("API_KEY")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            } else {
                None
            }
        })
}

fn resolve_existing_api_key(
    provider: Provider,
    existing: &ExistingSetup,
    provider_changed: bool,
) -> Option<(String, bool)> {
    if let Some(key) = env_api_key(provider) {
        return Some((key, true));
    }
    if !provider_changed {
        return existing.api_key.clone().map(|key| (key, false));
    }
    None
}

fn model_step_after_api_key(skipped_api_key: bool) -> WizardStep {
    if skipped_api_key {
        WizardStep::ApiKeyConfirm
    } else {
        WizardStep::ApiKey
    }
}

fn print_export_hint(provider: Provider, key: &str) {
    let var = provider_env_var(provider).unwrap_or("API_KEY");
    println!("\nTo persist the API key in your shell, run:\n  export {var}=\"{key}\"");
}

fn print_completion(
    saved: &SaveResult,
    provider: Provider,
    api_key: &Option<String>,
    key_from_env: bool,
) {
    match &saved.config {
        Some(path) => println!("\nConfiguration saved to {}", path.display()),
        None => println!("\nConfiguration applied for this session only."),
    }

    if let Some(path) = &saved.env {
        println!("API key saved to {}", path.display());
    } else if let Some(key) = api_key
        && !key_from_env
    {
        () = print_export_hint(provider, key);
    }
}

// ── Prompt steps ────────────────────────────────────────────────────────────

fn prompt_provider(existing: &ExistingSetup) -> Result<Option<Provider>, SetupError> {
    () = print_step(1, "Choose your AI provider");

    for (index, spec) in PROVIDER_SPECS.iter().enumerate() {
        () = print_bullet(&format!("{}. {}", index + 1, spec.wizard_label));
    }
    println!();

    let labels = PROVIDER_SPECS
        .iter()
        .map(|spec| spec.wizard_label)
        .collect::<Vec<_>>();

    let mut select = Select::new("Select a provider:", labels);

    if let Some(provider) = existing.provider
        && let Some(index) = PROVIDER_SPECS
            .iter()
            .position(|spec| spec.provider == provider)
    {
        select = select.with_starting_cursor(index);
        () = print_hint(&format!(
            "Current: {} — press Enter to keep",
            provider_label(provider)
        ));
    }

    () = print_navigation_hint(true);

    match select.prompt_skippable()? {
        None => Ok(None),
        Some(label) => {
            let provider = provider_from_label(label);
            () = print_ok(&format!("Provider: {label}"));
            Ok(Some(provider))
        }
    }
}

fn prompt_custom_api_key_confirm() -> Result<Option<bool>, SetupError> {
    () = print_navigation_hint(false);
    match Confirm::new("Does this endpoint require an API key?")
        .with_default(false)
        .prompt_skippable()?
    {
        None => Ok(None),
        Some(requires_api_key) => Ok(Some(requires_api_key)),
    }
}

fn prompt_api_key(
    provider: Provider,
    existing: &ExistingSetup,
    provider_changed: bool,
) -> Result<Option<(Option<String>, bool)>, SetupError> {
    () = print_step(2, "Enter your API key");
    let env_var = provider_env_var(provider).unwrap_or("API_KEY");
    let existing_key = resolve_existing_api_key(provider, existing, provider_changed);

    if let Some((_, from_env)) = &existing_key {
        if *from_env {
            () = print_hint(&format!(
                "(or set {env_var} in your shell and press Enter to skip)"
            ));
        } else {
            () = print_hint("(press Enter to keep the current key)");
        }
    } else {
        () = print_hint(&format!(
            "(or set {env_var} in your shell and press Enter to skip)"
        ));
    }
    println!();
    () = print_navigation_hint(false);

    let entered = match Text::new("API key:").prompt_skippable()? {
        None => return Ok(None),
        Some(value) => value,
    };
    let key = entered.trim();

    if key.is_empty() {
        if let Some((current, from_env)) = existing_key {
            let source = if from_env { env_var } else { "config file" };
            () = print_ok(&format!("Using key from {source}"));
            return Ok(Some((Some(current), from_env)));
        }
        if let Some(val) = env_api_key(provider) {
            () = print_ok(&format!("Using key from {env_var}"));
            return Ok(Some((Some(val), true)));
        }
        return Err(SetupError::Prompt(format!(
            "No API key provided. Set {env_var} or re-run the wizard."
        )));
    }

    () = print_ok("API key received");
    Ok(Some((Some(key.to_owned()), false)))
}

fn model_select_choices(
    provider: Provider,
    existing: &ExistingSetup,
    provider_changed: bool,
    default_model: &str,
) -> Vec<String> {
    let mut choices = known_models_for_provider(provider)
        .iter()
        .map(|model| (*model).to_string())
        .collect::<Vec<_>>();

    if !provider_changed
        && let Some(existing_model) = existing.model.as_deref().filter(|model| !model.is_empty())
        && !choices.iter().any(|choice| choice == existing_model)
    {
        () = choices.insert(0, existing_model.to_string());
    }

    if choices.is_empty() {
        choices.push(default_model.to_string());
    } else if !choices.iter().any(|choice| choice == default_model) {
        choices.insert(0, default_model.to_string());
    }

    () = choices.push(CUSTOM_MODEL_LABEL.to_string());
    choices
}

fn prompt_model(
    provider: Provider,
    existing: &ExistingSetup,
    provider_changed: bool,
) -> Result<Option<String>, SetupError> {
    let default_model = if provider_changed {
        default_model_for_provider(provider)
    } else {
        existing
            .model
            .as_deref()
            .unwrap_or(default_model_for_provider(provider))
    };

    () = print_step(3, "Choose a model");
    if !provider_changed && existing.model.is_some() {
        () = print_hint(&format!("Current: {default_model} — press Enter to keep"));
    }
    println!();

    let choices = model_select_choices(provider, existing, provider_changed, default_model);

    loop {
        let mut select = Select::new("Select a model:", choices.clone());
        if let Some(index) = choices.iter().position(|choice| choice == default_model) {
            select = select.with_starting_cursor(index);
        }
        () = print_navigation_hint(false);

        let choice = match select.prompt_skippable()? {
            None => return Ok(None),
            Some(choice) => choice,
        };

        let model = if choice == CUSTOM_MODEL_LABEL {
            loop {
                () = print_navigation_hint(false);
                let entered = match Text::new("Custom model:")
                    .with_default(default_model)
                    .prompt_skippable()?
                {
                    None => break None,
                    Some(value) => Some(value),
                };
                let Some(entered) = entered else {
                    continue;
                };
                let trimmed = entered.trim();
                break Some(if trimmed.is_empty() {
                    default_model.to_string()
                } else {
                    trimmed.to_owned()
                });
            }
        } else {
            Some(choice)
        };

        let Some(model) = model else {
            continue;
        };

        () = print_ok(&format!("Model: {model}"));
        return Ok(Some(model));
    }
}

fn prompt_base_url(
    existing: &ExistingSetup,
    provider_changed: bool,
) -> Result<Option<Option<String>>, SetupError> {
    let default = if provider_changed {
        "http://localhost:11434/v1"
    } else {
        existing
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434/v1")
    };

    println!();
    println!(
        "  {}: Enter the URL of your OpenAI-compatible API",
        "Base URL".bold()
    );
    () = print_hint("(e.g. http://localhost:11434/v1)");
    if existing.base_url.is_some() && !provider_changed {
        print_hint("(press Enter to keep the current URL)");
    }
    println!();
    () = print_navigation_hint(false);

    let value = match Text::new("Base URL:")
        .with_default(default)
        .prompt_skippable()?
    {
        None => return Ok(None),
        Some(value) => value,
    };

    let url = value.trim();
    if url.is_empty() {
        return Err(SetupError::Prompt(
            "No base URL provided. A base URL is required for custom providers.".into(),
        ));
    }

    () = print_ok(&format!("Base URL: {url}"));
    Ok(Some(Some(url.to_owned())))
}

const SAVE_CHOICE_USER: &str = "User-level — ~/.config/greatsage/config.toml";
const SAVE_CHOICE_PROJECT: &str = "Project — .greatsage/config.toml (this directory)";

fn prompt_save_location() -> Result<Option<SaveLocation>, SetupError> {
    () = print_step(4, "Save configuration");
    () = print_hint("Provider and model go to config.toml (`env!VAR` for keys).");
    () = print_hint("New API keys go to the paired `.env` in the same folder.");
    println!();
    () = print_navigation_hint(false);

    let choices = [SAVE_CHOICE_USER, SAVE_CHOICE_PROJECT];

    let choice = match Select::new("Where should configuration be saved?", choices.to_vec())
        .with_starting_cursor(0)
        .prompt_skippable()?
    {
        None => return Ok(None),
        Some(choice) => choice,
    };

    let location = if choice == SAVE_CHOICE_USER {
        SaveLocation::User
    } else {
        SaveLocation::Project
    };

    let summary = match location {
        SaveLocation::User => "~/.config/greatsage/config.toml",
        SaveLocation::Project => ".greatsage/config.toml",
    };
    () = print_ok(&format!("Save to: {summary}"));
    Ok(Some(location))
}

// ── Entry points ──────────────────────────────────────────────────────────

fn print_non_tty_guidance() {
    eprintln!("greatsage setup requires an interactive terminal (TTY).");
    eprintln!();
    eprintln!("Set a provider API key via environment variable, for example:");
    eprintln!("  export ANTHROPIC_API_KEY=your-key-here");
    eprintln!("  export OPENAI_API_KEY=your-key-here");
    eprintln!();
    eprintln!("Or create a config file:");
    eprintln!("  .greatsage/config.toml          (project-level)");
    eprintln!("  ~/.config/greatsage/config.toml  (user-level)");
    eprintln!();
    eprintln!("See `greatsage --help` for all provider env vars.");
}

pub fn run_wizard() -> Result<(), SetupError> {
    if !io::stdin().is_terminal() {
        () = print_non_tty_guidance();
        return Err(SetupError::NotInteractive);
    }

    () = print_welcome();

    let existing = load_existing_setup();

    let mut step = WizardStep::Provider;
    let mut prior_provider = existing.provider;
    let mut provider = existing.provider.unwrap_or_default();
    let mut provider_changed = false;
    let mut skipped_api_key = false;
    let mut api_key: Option<String> = None;
    let mut key_from_env = false;
    let mut model = String::new();
    let mut base_url: Option<String> = None;

    loop {
        match step {
            WizardStep::Provider => match prompt_provider(&existing)? {
                None => return Err(SetupError::Cancelled),
                Some(selected) => {
                    provider_changed = prior_provider != Some(selected);
                    prior_provider = Some(selected);
                    provider = selected;
                    skipped_api_key = false;
                    step = if provider == Provider::Custom {
                        WizardStep::ApiKeyConfirm
                    } else {
                        WizardStep::ApiKey
                    };
                }
            },
            WizardStep::ApiKeyConfirm => match prompt_custom_api_key_confirm()? {
                None => step = WizardStep::Provider,
                Some(requires_api_key) => {
                    if requires_api_key {
                        skipped_api_key = false;
                        step = WizardStep::ApiKey;
                    } else {
                        skipped_api_key = true;
                        api_key = None;
                        key_from_env = false;
                        () = print_hint("No API key needed for this endpoint.");
                        step = WizardStep::Model;
                    }
                }
            },
            WizardStep::ApiKey => match prompt_api_key(provider, &existing, provider_changed)? {
                None => {
                    step = if provider == Provider::Custom {
                        WizardStep::ApiKeyConfirm
                    } else {
                        WizardStep::Provider
                    };
                }
                Some((key, from_env)) => {
                    api_key = key;
                    key_from_env = from_env;
                    step = WizardStep::Model;
                }
            },
            WizardStep::Model => match prompt_model(provider, &existing, provider_changed)? {
                None => step = model_step_after_api_key(skipped_api_key),
                Some(selected) => {
                    model = selected;
                    step = if provider == Provider::Custom {
                        WizardStep::BaseUrl
                    } else {
                        WizardStep::Save
                    };
                }
            },
            WizardStep::BaseUrl => match prompt_base_url(&existing, provider_changed)? {
                None => step = WizardStep::Model,
                Some(url) => {
                    base_url = url;
                    step = WizardStep::Save;
                }
            },
            WizardStep::Save => match prompt_save_location()? {
                None => {
                    step = if provider == Provider::Custom {
                        WizardStep::BaseUrl
                    } else {
                        WizardStep::Model
                    };
                }
                Some(save_location) => {
                    () = apply_api_key_to_env(provider, api_key.as_deref());

                    let config = WizardConfig {
                        provider,
                        model: model.clone(),
                        base_url: base_url.clone(),
                        api_key: api_key.clone(),
                        key_from_env,
                    };

                    let saved = write_config(save_location, &config)?;
                    () = print_completion(&saved, provider, &api_key, key_from_env);

                    println!();
                    println!("  {}", "All set!".green().bold());
                    println!();

                    return Ok(());
                }
            },
        }
    }
}

pub fn offer_setup() -> Result<bool, SetupError> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }
    match Confirm::new("No API key configured. Run setup now?")
        .with_default(true)
        .prompt()
    {
        Ok(answer) => Ok(answer),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(false),
        Err(err) => Err(err.into()),
    }
}
