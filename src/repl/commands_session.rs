//! Session slash commands: /provider, /model, /save, /load, /compact, /retry.
//!
//! Async yoagent backends live in [`super::session_ops`].
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.

use {
    super::{
        dispatch::{AgentOp, AgentOpInvocation, DispatchResult, ReplDispatchCtx},
        help_data::push_usage_and_body_parts,
        model_cmd::{ModelAction, model_info_lines, model_list_lines, parse_model_args},
        route::CommandRoute,
        session_ops::{
            CompactArg, block_on_session, parse_compact_arg, preview_compact, resolve_session_path,
        },
        session_state::ReplSessionState,
    },
    crate::{
        agents::AgentConfig,
        config::Config,
        providers::{PROVIDER_SPECS, Provider, available_providers_line},
    },
    std::str::FromStr,
};

#[derive(Debug, Clone, Eq, PartialEq)]
enum ProviderAction {
    Show,
    List,
    Switch(String),
}

fn parse_provider_args(args: &str) -> ProviderAction {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        ProviderAction::Show
    } else if trimmed == "list" {
        ProviderAction::List
    } else {
        ProviderAction::Switch(trimmed.to_string())
    }
}

fn provider_list_lines(agent_config: &AgentConfig) -> Vec<String> {
    let mut lines = vec![format!("Providers (active: {})", agent_config.provider)];
    for spec in PROVIDER_SPECS {
        let name = spec.provider.to_string();
        let marker = if spec.provider == agent_config.provider {
            '▸'
        } else {
            ' '
        };
        () = lines.push(format!("{marker} {name}"));
    }
    () = lines.push(String::new());
    () = lines.push("Use: /provider <name> to switch".to_string());
    lines
}

fn reinstall_preserve_messages(
    agent_config: &AgentConfig,
    success_message: String,
) -> DispatchResult {
    DispatchResult::AgentOp(AgentOpInvocation {
        preamble: Vec::new(),
        op: AgentOp::ReinstallPreserveMessages {
            config: agent_config.clone(),
            success_message,
        },
    })
}

fn show_provider(agent_config: &AgentConfig) -> DispatchResult {
    DispatchResult::Handled {
        output: vec![
            format!("current provider: {}", agent_config.provider),
            String::from("usage: /provider <name> | list"),
            format!("available: {}", available_providers_line()),
        ],
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

fn show_model(agent_config: &AgentConfig) -> DispatchResult {
    DispatchResult::Handled {
        output: vec![
            format!("Current model: {}", agent_config.model),
            String::from("usage: /model <name> | list | info"),
        ],
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

fn retry(session: &ReplSessionState) -> DispatchResult {
    match &session.last_user_prompt {
        Some(prompt) => DispatchResult::ResendPrompt {
            prompt: prompt.clone(),
            hint: String::from("(retrying last input)"),
        },
        None => DispatchResult::Handled {
            output: vec![String::from("(nothing to retry — no previous input)")],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
    }
}

fn save(args: &str) -> DispatchResult {
    let path = resolve_session_path(if args.is_empty() { None } else { Some(args) });
    DispatchResult::AgentOp(AgentOpInvocation {
        preamble: Vec::new(),
        op: AgentOp::Save { path },
    })
}

fn load(args: &str) -> DispatchResult {
    let path = resolve_session_path(if args.is_empty() { None } else { Some(args) });
    DispatchResult::AgentOp(AgentOpInvocation {
        preamble: Vec::new(),
        op: AgentOp::Load { path },
    })
}

fn compact(args: &str, ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    match parse_compact_arg(args) {
        CompactArg::Default => DispatchResult::AgentOp(AgentOpInvocation {
            preamble: Vec::new(),
            op: AgentOp::Compact { keep_recent: None },
        }),
        CompactArg::KeepRecent(n) => DispatchResult::AgentOp(AgentOpInvocation {
            preamble: Vec::new(),
            op: AgentOp::Compact {
                keep_recent: Some(n),
            },
        }),
        CompactArg::Preview => {
            let output = match ctx.coding_agent {
                Some(agent) => vec![block_on_session(ctx.runtime, preview_compact(agent, None))],
                None => vec![String::from("No active agent.")],
            };
            DispatchResult::Handled {
                output,
                detail: Vec::new(),
                redraw_prompt: true,
                reinstall: None,
            }
        }
        CompactArg::Invalid(s) => DispatchResult::Handled {
            output: vec![
                format!("invalid argument: \"{s}\" — use a number, \"all\", or \"--preview\""),
                String::from("usage: /compact [N|all|--preview]"),
            ],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
    }
}

fn switch_provider(name: &str, agent_config: &mut AgentConfig, config: &Config) -> DispatchResult {
    let new_provider = match Provider::from_str(&name.to_lowercase()) {
        Ok(p) => p,
        Err(_) => {
            let (usage, detail) = push_usage_and_body_parts("/provider");
            return DispatchResult::Handled {
                output: vec![
                    format!("Unknown provider: {name}"),
                    usage,
                    String::from("Try /help /provider for details."),
                ],
                detail,
                redraw_prompt: true,
                reinstall: None,
            };
        }
    };

    agent_config.provider = new_provider;
    agent_config.model = new_provider.default_model().to_string();
    agent_config.api_key = config.get_api_key(Some(new_provider)).unwrap_or_default();
    let success_message = format!(
        "Switched to provider {new_provider} with model {} (conversation preserved).",
        agent_config.model
    );

    reinstall_preserve_messages(agent_config, success_message)
}

fn switch_model(model_name: String, agent_config: &mut AgentConfig) -> DispatchResult {
    agent_config.model = model_name;
    let success_message = format!(
        "Switched to model {} (conversation preserved).",
        agent_config.model
    );
    reinstall_preserve_messages(agent_config, success_message)
}

fn provider(args: &str, ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    match parse_provider_args(args) {
        ProviderAction::Show => show_provider(ctx.agent_config),
        ProviderAction::List => DispatchResult::Handled {
            output: provider_list_lines(ctx.agent_config),
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
        ProviderAction::Switch(name) => switch_provider(&name, ctx.agent_config, ctx.config),
    }
}

fn model(args: &str, ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    match parse_model_args(args) {
        ModelAction::Show => show_model(ctx.agent_config),
        ModelAction::ListAll => DispatchResult::Handled {
            output: model_list_lines(ctx.agent_config, ""),
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
        ModelAction::ListProvider { provider } => DispatchResult::Handled {
            output: model_list_lines(ctx.agent_config, &provider),
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
        ModelAction::Info { model } => {
            let name = model.as_deref().unwrap_or(&ctx.agent_config.model);
            DispatchResult::Handled {
                output: model_info_lines(name, ctx.agent_config),
                detail: Vec::new(),
                redraw_prompt: true,
                reinstall: None,
            }
        }
        ModelAction::Switch { model } => switch_model(model, ctx.agent_config),
    }
}

pub(super) fn dispatch(
    route: CommandRoute,
    args: &str,
    ctx: &mut ReplDispatchCtx<'_>,
) -> DispatchResult {
    match route {
        CommandRoute::Provider => provider(args, ctx),
        CommandRoute::Model => model(args, ctx),
        CommandRoute::Save => save(args),
        CommandRoute::Load => load(args),
        CommandRoute::Compact => compact(args, ctx),
        CommandRoute::Retry => retry(ctx.session),
        _ => DispatchResult::Unknown,
    }
}
