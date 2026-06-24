use {
    super::help_data::{format_help_detail, help_text},
    crate::agents::AgentConfig,
};

pub enum DispatchResult {
    Exit,
    Handled {
        output: Vec<String>,
        redraw_prompt: bool,
        reinstall: Option<AgentConfig>,
    },
    Unknown,
}

pub fn dispatch_slash_command(line: &str, agent_config: &mut AgentConfig) -> DispatchResult {
    let trimmed = line.trim();
    match trimmed {
        "/exit" | "/quit" => DispatchResult::Exit,
        "/clear" => DispatchResult::Handled {
            output: Vec::new(),
            redraw_prompt: true,
            reinstall: Some(agent_config.clone()),
        },
        s if s.starts_with("/model ") => handle_model(s, agent_config),
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