//! Session navigation slash commands: /history, /search, /mark, /jump, /marks, /export.

use {
    super::{
        dispatch::{AgentOp, AgentOpInvocation, DispatchResult, ReplDispatchCtx},
        route::CommandRoute,
        session_nav::{
            format_conversation_markdown, history_detail_lines, history_list_lines,
            parse_bookmark_name, parse_export_path, search_messages,
        },
        session_ops::block_on_session,
    },
    std::fs,
    yoagent::types::AgentMessage,
};

fn agent_messages(ctx: &ReplDispatchCtx<'_>) -> Vec<AgentMessage> {
    match ctx.coding_agent {
        Some(agent) => block_on_session(ctx.runtime, async {
            agent.lock().await.messages().to_vec()
        }),
        None => Vec::new(),
    }
}

fn handled(output: Vec<String>) -> DispatchResult {
    DispatchResult::Handled {
        output,
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

fn history(args: &str, ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    let messages = agent_messages(ctx);
    let trimmed = args.trim();
    if trimmed.starts_with("detail") {
        return handled(history_detail_lines(&messages));
    }
    if !trimmed.is_empty() {
        return handled(vec![
            format!("unknown subcommand: \"{trimmed}\""),
            String::from("usage: /history [detail]"),
        ]);
    }
    handled(history_list_lines(&messages))
}

fn search(args: &str, ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    let query = args.trim();
    if query.is_empty() {
        // yoyo: both lines dimmed with two-space indent, no blank between.
        return handled(vec![
            String::from("usage: /search <query>"),
            String::from("Search conversation history for messages containing <query>."),
        ]);
    }
    let messages = agent_messages(ctx);
    if messages.is_empty() {
        return handled(vec![String::from("(no messages to search)")]);
    }
    let results = search_messages(&messages, query);
    if results.is_empty() {
        return handled(vec![format!(
            "No matches for '{query}' in {} messages.",
            messages.len()
        )]);
    }
    let count = results.len();
    let es = if count == 1 { "" } else { "es" };
    let mut lines = vec![format!("{count} match{es} for '{query}':")];
    for (idx, role, preview) in results {
        lines.push(format!("  {idx:>3}. [{role}] {preview}"));
    }
    handled(lines)
}

fn mark(args: &str, ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    let name = match parse_bookmark_name(args) {
        Some(n) => n,
        None => {
            return handled(vec![
                String::from("usage: /mark <name>"),
                String::from("Save a bookmark at the current point in the conversation."),
                String::from("Use /jump <name> to return to this point later."),
            ]);
        }
    };

    let Some(agent) = ctx.coding_agent else {
        return handled(vec![String::from("No active agent.")]);
    };

    match block_on_session(ctx.runtime, async {
        let guard = agent.lock().await;
        let json = guard
            .save_messages()
            .map_err(|e| format!("error saving bookmark: {e}"))?;
        let count = guard.messages().len();
        Ok::<_, String>((json, count))
    }) {
        Ok((json, msg_count)) => {
            let overwriting = ctx.session.bookmarks.contains_key(&name);
            ctx.session.bookmarks.insert(name.clone(), json);
            let verb = if overwriting { "updated" } else { "saved" };
            handled(vec![format!(
                "✓ bookmark '{name}' {verb} ({msg_count} messages)"
            )])
        }
        Err(err) => handled(vec![err]),
    }
}

fn jump(args: &str, ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    if ctx.session_processing {
        return handled(vec![String::from(
            "(session is processing — wait for agent to finish before /jump)",
        )]);
    }

    let name = match parse_bookmark_name(args) {
        Some(n) => n,
        None => {
            return handled(vec![
                String::from("usage: /jump <name>"),
                String::from("Restore the conversation to a previously saved bookmark."),
                String::from("Messages added after the bookmark will be discarded."),
            ]);
        }
    };

    match ctx.session.bookmarks.get(&name) {
        Some(json) => DispatchResult::AgentOp(AgentOpInvocation {
            preamble: Vec::new(),
            op: AgentOp::Jump {
                json: json.clone(),
                config: ctx.agent_config.clone(),
                name,
            },
        }),
        None => {
            if ctx.session.bookmarks.is_empty() {
                handled(vec![
                    format!("bookmark '{name}' not found — no bookmarks saved yet."),
                    String::from("Use /mark <name> to save one."),
                ])
            } else {
                let mut names: Vec<&String> = ctx.session.bookmarks.keys().collect();
                names.sort();
                let available = names
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                handled(vec![
                    format!("bookmark '{name}' not found."),
                    format!("available: {available}"),
                ])
            }
        }
    }
}

fn marks(ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    if ctx.session.bookmarks.is_empty() {
        return handled(vec![
            String::from("(no bookmarks saved)"),
            String::from("Use /mark <name> to save a bookmark."),
        ]);
    }
    let mut names: Vec<&String> = ctx.session.bookmarks.keys().collect();
    names.sort();
    let mut lines = vec![String::from("Saved bookmarks:")];
    for name in names {
        lines.push(format!("  • {name}"));
    }
    handled(lines)
}

fn export(args: &str, ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    let path = parse_export_path(args);
    let messages = agent_messages(ctx);
    if messages.is_empty() {
        return handled(vec![String::from("(no messages to export)")]);
    }
    let markdown = format_conversation_markdown(&messages);
    match fs::write(path, &markdown) {
        Ok(()) => handled(vec![format!(
            "✓ conversation exported to {path} ({} messages)",
            messages.len()
        )]),
        Err(e) => handled(vec![format!("error writing to {path}: {e}")]),
    }
}

pub(super) fn dispatch(
    route: CommandRoute,
    args: &str,
    ctx: &mut ReplDispatchCtx<'_>,
) -> DispatchResult {
    match route {
        CommandRoute::History => history(args, ctx),
        CommandRoute::Search => search(args, ctx),
        CommandRoute::Mark => mark(args, ctx),
        CommandRoute::Jump => jump(args, ctx),
        CommandRoute::Marks => marks(ctx),
        CommandRoute::Export => export(args, ctx),
        _ => DispatchResult::Unknown,
    }
}
