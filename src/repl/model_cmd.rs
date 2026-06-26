//! `/model` subcommand parsing and read-only list/info output.
//!
//! Switching models stays in [`super::commands_session`]; this module handles
//! `list`, `list <provider>`, and `info [name]` using [`crate::providers::PROVIDER_SPECS`].

use {
    super::cost::format_pricing_lines,
    crate::{
        agents::AgentConfig,
        providers::{PROVIDER_SPECS, Provider, available_provider_names, available_providers_line},
    },
    std::str::FromStr,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAction {
    Show,
    ListAll,
    ListProvider { provider: String },
    Info { model: Option<String> },
    Switch { model: String },
}

pub fn parse_model_args(args: &str) -> ModelAction {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return ModelAction::Show;
    }
    if trimmed == "list" {
        return ModelAction::ListAll;
    }
    if let Some(rest) = trimmed.strip_prefix("list ") {
        let provider = rest.trim();
        if provider.is_empty() {
            return ModelAction::ListAll;
        }
        return ModelAction::ListProvider {
            provider: provider.to_string(),
        };
    }
    if trimmed == "info" {
        return ModelAction::Info { model: None };
    }
    if let Some(rest) = trimmed.strip_prefix("info ") {
        let name = rest.trim();
        return ModelAction::Info {
            model: if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            },
        };
    }
    ModelAction::Switch {
        model: trimmed.to_string(),
    }
}

fn format_context_size(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        let m = tokens as f64 / 1_000_000.0;
        if (m - m.round()).abs() < 0.001 {
            format!("{}M tokens", m as u64)
        } else {
            format!("{m:.1}M tokens")
        }
    } else {
        format!("{}k tokens", tokens / 1_000)
    }
}

pub fn model_context_window(model: &str) -> Option<u64> {
    let model = model.to_lowercase();
    if model.contains("claude") {
        if model.contains("sonnet-4") || model.contains("sonnet-4.") {
            return Some(1_000_000);
        }
        if model.contains("opus") {
            if model.contains("4-6")
                || model.contains("4.6")
                || model.contains("4-7")
                || model.contains("4.7")
            {
                return Some(1_000_000);
            }
            return Some(200_000);
        }
        return Some(200_000);
    }
    if model.contains("codex-mini") {
        return Some(192_000);
    }
    if model.contains("gpt-4.1") {
        return Some(1_048_576);
    }
    if model.contains("gpt-4o") {
        return Some(128_000);
    }
    if model.contains("gpt-5") {
        return Some(1_048_576);
    }
    if model.starts_with("o3") || model.starts_with("o4") {
        return Some(200_000);
    }
    if model.contains("gemini-3") || model.contains("gemini-2.5") || model.contains("gemini-2.0") {
        return Some(1_048_576);
    }
    if model.contains("grok") {
        return Some(131_072);
    }
    if model.contains("llama-4") {
        return Some(128_000);
    }
    if model.contains("deepseek") {
        return Some(128_000);
    }
    if model.contains("mistral") || model.contains("codestral") {
        return Some(128_000);
    }
    None
}

pub fn find_provider_for_model(model: &str) -> Option<Provider> {
    for spec in PROVIDER_SPECS {
        if spec.known_models.contains(&model) {
            return Some(spec.provider);
        }
    }
    let lower = model.to_lowercase();
    if lower.contains("claude") {
        return Some(Provider::Anthropic);
    }
    if lower.starts_with("gpt-") || lower.starts_with("o3") || lower.starts_with("o4") {
        return Some(Provider::OpenAi);
    }
    if lower.contains("gemini") {
        return Some(Provider::Google);
    }
    if lower.contains("grok") {
        return Some(Provider::Xai);
    }
    if lower.contains("deepseek") {
        return Some(Provider::DeepSeek);
    }
    if lower.contains("mistral") || lower.contains("codestral") {
        return Some(Provider::Mistral);
    }
    None
}

fn resolve_list_providers(filter: &str) -> Result<Vec<Provider>, String> {
    if filter.is_empty() {
        return Ok(available_provider_names()
            .into_iter()
            .filter_map(|name| Provider::from_str(&name).ok())
            .collect());
    }
    Provider::from_str(&filter.to_lowercase())
        .map(|p| vec![p])
        .map_err(|_| {
            format!(
                "unknown provider: {filter}. available: {}",
                available_providers_line()
            )
        })
}

/// Search needles for `/model list <filter>` when `filter` is not a provider id.
fn model_list_filter_needles(filter: &str) -> Vec<String> {
    let lower = filter.trim().to_lowercase();
    let mut needles = vec![lower.clone()];
    if let Some(rest) = lower.strip_prefix("gpt-") {
        needles.push(rest.to_string());
    }
    if let Some(rest) = lower.strip_prefix("openai/") {
        needles.push(rest.to_string());
    }
    () = needles.sort();
    () = needles.dedup();
    needles
}

fn model_matches_filter(model: &str, needles: &[String]) -> bool {
    let lower = model.to_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

/// Providers with catalog models matching a non-provider filter (e.g. `gpt-o4` → `o4`).
fn providers_with_matching_models(filter: &str) -> Vec<(Provider, Vec<&'static str>)> {
    let needles = model_list_filter_needles(filter);
    let mut grouped = Vec::new();
    for spec in PROVIDER_SPECS {
        let matched: Vec<&'static str> = spec
            .known_models
            .iter()
            .copied()
            .filter(|model| model_matches_filter(model, &needles))
            .collect();
        if !matched.is_empty() {
            grouped.push((spec.provider, matched));
        }
    }
    grouped
}

pub fn model_list_lines(agent_config: &AgentConfig, filter: &str) -> Vec<String> {
    let filter = filter.trim();

    if filter.is_empty() || Provider::from_str(&filter.to_lowercase()).is_ok() {
        let providers = match resolve_list_providers(filter) {
            Ok(providers) => providers,
            Err(message) => return vec![message],
        };

        let title = format!("Models by provider (active: {})", agent_config.model);

        let mut lines = vec![title];
        for provider in providers {
            append_provider_models(&mut lines, agent_config, provider, None);
        }
        () = lines.push(String::new());
        () = lines.push("Use: /model <name> to switch".to_string());
        return lines;
    }

    let groups = providers_with_matching_models(filter);
    if groups.is_empty() {
        return vec![format!(
            "No models match \"{filter}\". Try /model list <provider> (e.g. openai) or /model info {filter}"
        )];
    }

    let mut lines = vec![format!(
        "Models matching \"{filter}\" (active: {})",
        agent_config.model
    )];
    for (provider, models) in groups {
        append_provider_models(&mut lines, agent_config, provider, Some(&models));
    }
    () = lines.push(String::new());
    () = lines.push(String::from("Use: /model <name> to switch"));
    lines
}

fn append_provider_models(
    lines: &mut Vec<String>,
    agent_config: &AgentConfig,
    provider: Provider,
    models: Option<&[&'static str]>,
) {
    let spec = provider.spec();
    let default_model = spec.default_model;
    let provider_name = provider.to_string();
    let header = if provider == agent_config.provider {
        format!("* {provider_name}")
    } else {
        provider_name
    };
    () = lines.push(String::new());
    () = lines.push(header);

    let catalog = models.unwrap_or(spec.known_models);
    if catalog.is_empty() {
        return;
    }
    for model in catalog {
        let is_active = *model == agent_config.model.as_str();
        let is_default = *model == default_model;
        let marker = if is_active { '▸' } else { ' ' };
        let suffix = if is_default { "  (default)" } else { "" };
        () = lines.push(format!("{marker} {model}{suffix}"));
    }
}

fn model_info_header(model_name: &str) -> Vec<String> {
    let separator = "─".repeat(model_name.len() + 6);
    vec![
        separator.clone(),
        format!("── {model_name} ──"),
        separator,
        String::new(),
    ]
}

pub fn model_info_lines(model_name: &str, agent_config: &AgentConfig) -> Vec<String> {
    let mut lines = Vec::with_capacity(12);
    () = lines.push(String::new());
    () = lines.extend(model_info_header(model_name));

    match find_provider_for_model(model_name) {
        Some(provider) => lines.push(format!("Provider:  {provider}")),
        None => lines.push("Provider:  unknown".to_string()),
    }

    match model_context_window(model_name) {
        Some(ctx) => lines.push(format!("Context:   {}", format_context_size(ctx))),
        None => lines.push("Context:   unknown".to_string()),
    }

    let pricing_provider = find_provider_for_model(model_name).unwrap_or(agent_config.provider);
    lines.extend(format_pricing_lines(pricing_provider, model_name));

    if let Some(provider) = find_provider_for_model(model_name)
        && provider.default_model() == model_name
    {
        () = lines.push(format!("Default:   ✓ (for {provider})"));
    }

    if model_name == agent_config.model {
        () = lines.push("Active:    ✓".to_string());
    }

    lines
}
