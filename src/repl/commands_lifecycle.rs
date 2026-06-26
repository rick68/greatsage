//! Lifecycle commands: /quit, /exit, /clear, /clear!.
//!
//! [`dispatch`] is the sole entry at the bottom of this module.

use {
    super::{
        dispatch::{DispatchResult, ReplDispatchCtx},
        route::CommandRoute,
    },
    crate::agents::AgentConfig,
};

const CLEAR_CONFIRM_MESSAGE_THRESHOLD: usize = 4;

fn format_token_count(tokens: u64) -> String {
    if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Returns `None` when the conversation is small enough to clear without prompting.
pub(super) fn clear_confirmation_message(message_count: usize, token_count: u64) -> Option<String> {
    if message_count <= CLEAR_CONFIRM_MESSAGE_THRESHOLD {
        return None;
    }
    Some(format!(
        "Clear {message_count} messages (~{} tokens)? [y/N]",
        format_token_count(token_count)
    ))
}

fn clear_handled_result(agent_config: &AgentConfig, force: bool) -> DispatchResult {
    let message = if force {
        "(conversation force-cleared)"
    } else {
        "(conversation cleared)"
    };
    DispatchResult::Handled {
        output: vec![String::from(message)],
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: Some(agent_config.clone()),
    }
}

pub(super) fn dispatch(route: CommandRoute, ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    match route {
        CommandRoute::Quit | CommandRoute::Exit => DispatchResult::Exit,
        CommandRoute::Clear => {
            if let Some((message_count, token_count)) = ctx.clear_stats
                && let Some(prompt) = clear_confirmation_message(message_count, token_count)
            {
                return DispatchResult::AwaitClearConfirm { prompt };
            }
            clear_handled_result(ctx.agent_config, false)
        }
        CommandRoute::ClearForce => clear_handled_result(ctx.agent_config, true),
        _ => DispatchResult::Unknown,
    }
}
