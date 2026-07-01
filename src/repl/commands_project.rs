//! Project-related slash commands: `/context`, `/init`.

use {
    super::{
        context_display::{context_files_lines, context_list_lines, context_system_lines},
        dispatch::{DispatchResult, ReplDispatchCtx},
        session_dashboard::estimate_tokens,
        session_ops::block_on_session,
    },
    crate::{
        agents::SYSTEM_PROMPT,
        project_init::{detect_ai_config_files, init_greatsage_md, init_repl_output_lines},
    },
    std::path::Path,
    yoagent::types::AgentMessage,
};

fn unknown_subcommand(args: &str) -> DispatchResult {
    DispatchResult::Handled {
        output: vec![
            format!("unknown subcommand: \"{}\"", args.trim()),
            "usage: /context [system|files]".to_string(),
        ],
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

pub(super) fn dispatch_context(args: &str, ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    let trimmed = args.trim();
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let system_prompt = if ctx.agent_config.system_prompt.trim().is_empty() {
        SYSTEM_PROMPT
    } else {
        ctx.agent_config.system_prompt.as_str()
    };

    let output = if trimmed.starts_with("system") {
        context_system_lines(system_prompt, estimate_tokens)
    } else if trimmed.starts_with("files") {
        let messages: Vec<AgentMessage> = ctx
            .coding_agent
            .map(|agent| {
                block_on_session(ctx.runtime, async {
                    agent.lock().await.messages().to_vec()
                })
            })
            .unwrap_or_default();
        context_files_lines(&messages)
    } else if trimmed.is_empty() {
        context_list_lines(&cwd, system_prompt, ctx.bare)
    } else {
        return unknown_subcommand(trimmed);
    };

    DispatchResult::Handled {
        output,
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

pub(super) fn dispatch_init(args: &str, _ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    if !args.trim().is_empty() {
        return DispatchResult::Handled {
            output: vec![
                format!("invalid argument: \"{}\"", args.trim()),
                String::from("usage: /init"),
            ],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        };
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let ai_names: Vec<&str> = detect_ai_config_files(&cwd)
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    let outcome = init_greatsage_md(&cwd);
    let output = init_repl_output_lines(&outcome, &ai_names);

    DispatchResult::Handled {
        output,
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}
