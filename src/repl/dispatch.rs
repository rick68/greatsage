//! Slash-command router and shared dispatch types.
//!
//! Per-domain handlers live in flat `commands_*.rs` modules; wire new commands in
//! [`route`] and the matching domain `dispatch` function.
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.

use {
    super::{
        commands_help, commands_info, commands_lifecycle, commands_memory, commands_project,
        commands_session,
        route::{CommandRoute, route_command},
        session_dashboard::SessionDashboardSnapshot,
        session_state::ReplSessionState,
        suggest::suggest_command,
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
    pub coding_agent: Option<&'a crate::agents::CodingAgent>,
    pub runtime: &'a tokio::runtime::Runtime,
    /// Precomputed for `/status`, `/tokens`, `/cost` when dispatch runs from the stdin loop.
    pub dashboard: Option<SessionDashboardSnapshot>,
    /// CLI `-b` / `--bare`: project context is not loaded into the agent.
    pub bare: bool,
}

pub(super) enum AgentOp {
    Save {
        path: PathBuf,
    },
    Load {
        path: PathBuf,
    },
    Compact {
        keep_recent: Option<usize>,
    },
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

pub(super) struct UnknownSlashFeedback {
    pub typed: String,
    pub suggestion: Option<&'static str>,
}

pub(super) fn build_unknown_slash_feedback(line: &str) -> UnknownSlashFeedback {
    let typed = line.split_whitespace().next().unwrap_or(line).to_string();
    UnknownSlashFeedback {
        suggestion: suggest_command(line),
        typed,
    }
}

pub(super) fn dispatch_slash_command(
    line: &str,
    agent_config: &mut AgentConfig,
    session: &ReplSessionState,
    config: &Config,
    clear_stats: Option<(usize, u64)>,
    coding_agent: Option<&crate::agents::CodingAgent>,
    runtime: &tokio::runtime::Runtime,
    dashboard: Option<SessionDashboardSnapshot>,
    bare: bool,
) -> DispatchResult {
    let (cmd, args) = command_name_and_args(line);
    let route = route_command(cmd);
    let mut ctx = ReplDispatchCtx {
        agent_config,
        session,
        config,
        clear_stats,
        coding_agent,
        runtime,
        dashboard,
        bare,
    };

    match route {
        CommandRoute::Help => commands_help::help(args),
        route if route.is_lifecycle() => commands_lifecycle::dispatch(route, &mut ctx),
        route if route.is_session() => commands_session::dispatch(route, args, &mut ctx),
        CommandRoute::Context => commands_project::dispatch_context(args, &ctx),
        CommandRoute::Init => commands_project::dispatch_init(args, &ctx),
        CommandRoute::Remember => commands_memory::dispatch_remember(args, &ctx),
        CommandRoute::Memories => commands_memory::dispatch_memories(args, &ctx),
        CommandRoute::Forget => commands_memory::dispatch_forget(args, &ctx),
        route if route.is_info() => commands_info::dispatch(route, args, &ctx),
        CommandRoute::UnknownSlash | CommandRoute::NotSlash => DispatchResult::Unknown,
        _ => DispatchResult::Unknown,
    }
}
