use {
    super::{
        agent_session::resolve_session_path,
        compact_parse::{parse_compact_arg, CompactArg},
        help_data::{format_help_detail, help_text},
        session_state::ReplSessionState,
    },
    crate::{
        agents::AgentConfig,
        config::Config,
        providers::Provider,
    },
    std::{path::PathBuf, str::FromStr},
};

pub enum AgentOp {
    Save { path: PathBuf },
    Load { path: PathBuf },
    Compact { arg: CompactArg },
    ReinstallPreserveMessages(AgentConfig),
}

pub enum DispatchResult {
    Exit,
    Handled {
        output: Vec<String>,
        redraw_prompt: bool,
        reinstall: Option<AgentConfig>,
    },
    ResendPrompt(String),
    AgentOp(AgentOp),
    Unknown,
}

pub fn dispatch_slash_command(
    line: &str,
    agent_config: &mut AgentConfig,
    session: &ReplSessionState,
    config: &Config,
) -> DispatchResult {
    let trimmed = line.trim();
    match trimmed {
        "/exit" | "/quit" => DispatchResult::Exit,
        "/clear" => DispatchResult::Handled {
            output: Vec::new(),
            redraw_prompt: true,
            reinstall: Some(agent_config.clone()),
        },
        "/retry" => handle_retry(session),
        "/provider" => show_provider(agent_config),
        s if s.starts_with("/provider ") => handle_provider(s, agent_config, config),
        s if s.starts_with("/model ") => handle_model(s, agent_config),
        "/save" => DispatchResult::AgentOp(AgentOp::Save {
            path: resolve_session_path(None),
        }),
        s if s.starts_with("/save ") => {
            let path = s.trim_start_matches("/save ").trim();
            DispatchResult::AgentOp(AgentOp::Save {
                path: resolve_session_path(Some(path)),
            })
        }
        "/load" => DispatchResult::AgentOp(AgentOp::Load {
            path: resolve_session_path(None),
        }),
        s if s.starts_with("/load ") => {
            let path = s.trim_start_matches("/load ").trim();
            DispatchResult::AgentOp(AgentOp::Load {
                path: resolve_session_path(Some(path)),
            })
        }
        "/compact" => DispatchResult::AgentOp(AgentOp::Compact {
            arg: CompactArg::Default,
        }),
        s if s.starts_with("/compact ") => {
            let arg_str = s.trim_start_matches("/compact ").trim();
            DispatchResult::AgentOp(AgentOp::Compact {
                arg: parse_compact_arg(arg_str),
            })
        }
        "/help" => DispatchResult::Handled {
            output: vec![help_text()],
            redraw_prompt: true,
            reinstall: None,
        },
        s if s.starts_with("/help ") => {
            let arg = s.trim_start_matches("/help ").trim();
            let output = if arg.is_empty() {
                help_text()
            } else {
                format_help_detail(arg)
            };
            DispatchResult::Handled {
                output: vec![output],
                redraw_prompt: true,
                reinstall: None,
            }
        }
        _ => DispatchResult::Unknown,
    }
}

fn show_provider(agent_config: &AgentConfig) -> DispatchResult {
    DispatchResult::Handled {
        output: vec![format!("Current provider: {}", agent_config.provider)],
        redraw_prompt: true,
        reinstall: None,
    }
}

fn handle_provider(
    line: &str,
    agent_config: &mut AgentConfig,
    config: &Config,
) -> DispatchResult {
    let name = line.trim_start_matches("/provider ").trim();
    if name.is_empty() {
        return show_provider(agent_config);
    }

    let new_provider = match Provider::from_str(name) {
        Ok(p) => p,
        Err(_) => {
            return DispatchResult::Handled {
                output: vec![format!("Unknown provider: {name}. Try /help /provider.")],
                redraw_prompt: true,
                reinstall: None,
            };
        }
    };

    agent_config.provider = new_provider;
    agent_config.model = new_provider.default_model().to_string();
    agent_config.api_key = config
        .get_api_key(Some(new_provider))
        .unwrap_or_default();

    DispatchResult::AgentOp(AgentOp::ReinstallPreserveMessages(agent_config.clone()))
}

fn handle_retry(session: &ReplSessionState) -> DispatchResult {
    match &session.last_user_prompt {
        Some(prompt) => DispatchResult::ResendPrompt(prompt.clone()),
        None => DispatchResult::Handled {
            output: vec!["No previous prompt to retry.".to_string()],
            redraw_prompt: true,
            reinstall: None,
        },
    }
}

fn handle_model(line: &str, agent_config: &mut AgentConfig) -> DispatchResult {
    let new_model = line.trim_start_matches("/model ").trim();
    if new_model.is_empty() {
        return DispatchResult::Handled {
            output: Vec::new(),
            redraw_prompt: false,
            reinstall: None,
        };
    }

    agent_config.model = String::from(new_model);
    DispatchResult::Handled {
        output: Vec::new(),
        redraw_prompt: true,
        reinstall: Some(agent_config.clone()),
    }
}

pub fn unknown_command_message() -> &'static str {
    "Unknown command. Try /help."
}