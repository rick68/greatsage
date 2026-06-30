//! Read-only session dashboard slash commands: `/status`, `/tokens`, `/cost`.

use {
    super::{
        cost::cost_output_lines,
        dispatch::{DispatchResult, ReplDispatchCtx},
        route::CommandRoute,
        session_dashboard::{
            SessionDashboardSnapshot, format_context_fill, format_elapsed, tokens_output_lines,
        },
        session_ops::block_on_session,
    },
    crate::{agents::SYSTEM_PROMPT, utils::format_usage_line},
    std::env,
    yoagent::types::AgentMessage,
};

fn reject_args(command: &str, args: &str) -> Option<DispatchResult> {
    if args.trim().is_empty() {
        return None;
    }
    Some(DispatchResult::Handled {
        output: vec![
            format!("invalid argument: \"{}\"", args.trim()),
            format!("usage: {command}"),
        ],
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    })
}

fn snapshot(ctx: &ReplDispatchCtx<'_>) -> SessionDashboardSnapshot {
    ctx.dashboard.clone().unwrap_or(SessionDashboardSnapshot {
        turn_count: 0,
        message_count: 0,
        usage: yoagent::types::Usage::default(),
        context_used: 0,
        context_max: super::model_cmd::model_context_window(&ctx.agent_config.model),
        show_compaction_note: false,
        started_at_ms: None,
        cwd: None,
    })
}

fn status(args: &str, ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    if let Some(result) = reject_args("/status", args) {
        return result;
    }

    let snap = snapshot(ctx);
    let cwd = snap
        .cwd
        .clone()
        .or_else(|| {
            env::current_dir()
                .ok()
                .map(|path| path.display().to_string())
        })
        .unwrap_or_else(|| ".".to_string());

    let mut output = vec![
        format!("model: {}", ctx.agent_config.model),
        format!("provider: {}", ctx.agent_config.provider),
        format!("cwd: {cwd}"),
        format!("turns: {}", snap.turn_count),
    ];

    if let Some(line) = format_usage_line(&snap.usage) {
        output.push(line);
    } else {
        output.push("tokens: 0 in / 0 out".to_string());
    }

    () = output.push(format!(
        "context: {}",
        format_context_fill(snap.context_used, snap.context_max)
    ));

    if let Some(elapsed) = format_elapsed(snap.started_at_ms) {
        () = output.push(format!("elapsed: {elapsed}"));
    }

    DispatchResult::Handled {
        output,
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

fn tokens(args: &str, ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    if let Some(result) = reject_args("/tokens", args) {
        return result;
    }

    let snap = snapshot(ctx);
    let provider = ctx.agent_config.provider;
    let model = ctx.agent_config.model.as_str();
    let system_prompt = if ctx.agent_config.system_prompt.trim().is_empty() {
        SYSTEM_PROMPT
    } else {
        ctx.agent_config.system_prompt.as_str()
    };

    let messages: Vec<AgentMessage> = ctx
        .coding_agent
        .map(|agent| {
            block_on_session(ctx.runtime, async {
                agent.lock().await.messages().to_vec()
            })
        })
        .unwrap_or_default();

    let output = tokens_output_lines(&snap, &messages, system_prompt, provider, model);

    DispatchResult::Handled {
        output,
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

fn cost(args: &str, ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    if let Some(result) = reject_args("/cost", args) {
        return result;
    }

    let snap = snapshot(ctx);
    let provider = ctx.agent_config.provider;
    let model = ctx.agent_config.model.as_str();

    let messages: Vec<AgentMessage> = ctx
        .coding_agent
        .map(|agent| {
            block_on_session(ctx.runtime, async {
                agent.lock().await.messages().to_vec()
            })
        })
        .unwrap_or_default();

    let output = cost_output_lines(&snap.usage, provider, model, &messages);

    DispatchResult::Handled {
        output,
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

pub(super) fn dispatch(
    route: CommandRoute,
    args: &str,
    ctx: &ReplDispatchCtx<'_>,
) -> DispatchResult {
    match route {
        CommandRoute::Status => status(args, ctx),
        CommandRoute::Tokens => tokens(args, ctx),
        CommandRoute::Cost => cost(args, ctx),
        _ => DispatchResult::Unknown,
    }
}
