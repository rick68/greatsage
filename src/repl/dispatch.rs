//! Slash-command router and shared dispatch types.
//!
//! Per-domain handlers live in flat `commands_*.rs` modules; wire new commands in
//! [`route`] and the matching domain `dispatch` function.
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.

use {
    super::{
        commands_help, commands_lifecycle, commands_session,
        route::{CommandRoute, route_command},
        session_state::ReplSessionState,
    },
    crate::{agents::AgentConfig, config::Config},
    std::path::PathBuf,
};

/// Shared inputs for slash-command handlers (grows without touching Bevy systems).
pub(super) struct ReplDispatchCtx<'a> {
    pub agent_config: &'a mut AgentConfig,
    pub session: &'a ReplSessionState,
    pub config: &'a Config,
    /// `(message_count, token_count)` for `/clear` confirmation; `None` when agent unavailable.
    pub clear_stats: Option<(usize, u64)>,
}

pub(super) enum AgentOp {
    Save {
        path: PathBuf,
    },
    Load {
        path: PathBuf,
    },
    Compact,
    ReinstallPreserveMessages {
        config: AgentConfig,
        success_message: String,
    },
}

pub(super) struct AgentOpInvocation {
    pub op: AgentOp,
    pub preamble: Vec<String>,
}

pub(super) enum DispatchResult {
    Exit,
    Handled {
        output: Vec<String>,
        /// Explanatory lines printed flush-left (no two-space REPL indent).
        detail: Vec<String>,
        redraw_prompt: bool,
        reinstall: Option<AgentConfig>,
    },
    ResendPrompt {
        prompt: String,
        hint: String,
    },
    AgentOp(AgentOpInvocation),
    /// `/clear` needs y/n before reinstalling the agent.
    AwaitClearConfirm {
        prompt: String,
    },
    Unknown,
}

pub(super) fn command_name_and_args(line: &str) -> (&str, &str) {
    let trimmed = line.trim();
    match trimmed.split_once(char::is_whitespace) {
        Some((cmd, args)) => (cmd, args.trim()),
        None => (trimmed, ""),
    }
}

pub(super) fn unknown_command_message() -> &'static str {
    "Unknown command. Try /help."
}

pub(super) fn dispatch_slash_command(
    line: &str,
    agent_config: &mut AgentConfig,
    session: &ReplSessionState,
    config: &Config,
    clear_stats: Option<(usize, u64)>,
) -> DispatchResult {
    let (cmd, args) = command_name_and_args(line);
    let route = route_command(cmd);
    let mut ctx = ReplDispatchCtx {
        agent_config,
        session,
        config,
        clear_stats,
    };

    match route {
        CommandRoute::Help => commands_help::help(args),
        route if route.is_lifecycle() => commands_lifecycle::dispatch(route, &mut ctx),
        route if route.is_session() => commands_session::dispatch(route, args, &mut ctx),
        CommandRoute::UnknownSlash | CommandRoute::NotSlash => DispatchResult::Unknown,
        _ => DispatchResult::Unknown,
    }
}
