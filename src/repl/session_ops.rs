//! yoagent save / load / compact backends for session commands.
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.

use {
    super::path_display::session_status_path,
    crate::agents::{AgentConfig, CodingAgent},
    std::{
        env, fs,
        future::Future,
        path::{Path, PathBuf},
    },
    tokio::{runtime::Runtime, task},
    yoagent::{
        context::{ContextConfig, compact_messages, total_tokens},
        types::{AgentMessage, Content, Message},
    },
};

/// Bridge sync REPL dispatch to async yoagent locks on the app Tokio runtime.
pub fn block_on_session<T>(runtime: &Runtime, future: impl Future<Output = T>) -> T {
    runtime.handle().block_on(future)
}

/// Bound wait for the agent mutex (exit / auto-save must not hang if a turn holds the lock).
const AGENT_LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Abort in-flight yoagent work if the lock is available quickly.
pub fn abort_agent_best_effort(runtime: &Runtime, agent: &CodingAgent) {
    let agent = agent.clone();
    let _ = runtime.handle().block_on(async move {
        match tokio::time::timeout(AGENT_LOCK_TIMEOUT, agent.lock()).await {
            Ok(guard) => {
                guard.abort();
            }
            Err(_) => {
                // Still streaming / locked — skip; cancel tokens / task teardown continue.
            }
        }
    });
}

pub const DEFAULT_SESSION_FILENAME: &str = "greatsage-session.json";
pub const LAST_SESSION_REL_PATH: &str = ".greatsage/last-session.json";

fn default_session_path() -> PathBuf {
    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(DEFAULT_SESSION_FILENAME)
}

/// Auto-save and `--continue` target under cwd.
pub fn last_session_path(cwd: &Path) -> PathBuf {
    cwd.join(LAST_SESSION_REL_PATH)
}

fn load_status_line(path: &Path, count: usize) -> String {
    format!(
        "(session loaded from {}, {count} messages)",
        session_status_path(path, default_session_path()),
    )
}

fn user_prompt_from_messages(messages: &[AgentMessage]) -> Option<String> {
    messages.iter().rev().find_map(|msg| {
        let AgentMessage::Llm(Message::User { content, .. }) = msg else {
            return None;
        };
        let text: String = content
            .iter()
            .filter_map(|block| match block {
                Content::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        if text.is_empty() { None } else { Some(text) }
    })
}

/// Last non-slash user prompt for `/retry` after `/load` or `--continue`.
pub fn last_user_prompt_from_messages(messages: &[AgentMessage]) -> Option<String> {
    user_prompt_from_messages(messages)
}

/// Snapshot of yoagent message stats for `/clear` confirmation.
pub(super) async fn agent_message_stats(agent: &CodingAgent) -> (usize, u64) {
    let guard = agent.lock().await;
    let messages = guard.messages();
    (messages.len(), total_tokens(messages) as u64)
}

fn nothing_to_compact_message(message_count: usize, token_count: u64) -> String {
    format!("(nothing to compact — {message_count} messages, ~{token_count} tokens)")
}

/// Result of parsing a `/compact` argument (yoyo-aligned).
#[derive(Debug, PartialEq, Eq)]
pub enum CompactArg {
    Default,
    KeepRecent(usize),
    Preview,
    Invalid(String),
}

pub fn parse_compact_arg(arg: &str) -> CompactArg {
    let arg = arg.trim();
    if arg.is_empty() {
        return CompactArg::Default;
    }
    if arg.eq_ignore_ascii_case("all") {
        return CompactArg::KeepRecent(2);
    }
    if arg == "--preview" || arg.eq_ignore_ascii_case("preview") {
        return CompactArg::Preview;
    }
    match arg.parse::<usize>() {
        Ok(n) => CompactArg::KeepRecent(n.max(2)),
        Err(_) => CompactArg::Invalid(arg.to_string()),
    }
}

fn compact_context_config(keep_recent: Option<usize>) -> ContextConfig {
    match keep_recent {
        None => ContextConfig::default(),
        Some(kr) => ContextConfig {
            max_context_tokens: 0,
            system_prompt_tokens: 0,
            keep_recent: kr,
            ..ContextConfig::default()
        },
    }
}

fn compact_summary(
    before_count: usize,
    before_tokens: u64,
    after_count: usize,
    after_tokens: u64,
) -> String {
    format!(
        "Compacted: {before_count} → {after_count} messages, {before_tokens} → {after_tokens} tokens."
    )
}

fn preview_lines(
    before_count: usize,
    before_tokens: u64,
    after_count: usize,
    after_tokens: u64,
) -> String {
    let savings = before_tokens.saturating_sub(after_tokens);
    format!(
        "Compact preview:\n  Current: {before_count} messages, ~{before_tokens} tokens\n  After:   {after_count} messages, ~{after_tokens} tokens (estimated)\n  Savings: ~{savings} tokens"
    )
}

pub(super) fn resolve_session_path(path: Option<&str>) -> PathBuf {
    match path.filter(|p| !p.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => default_session_path(),
    }
}

pub(super) async fn save_messages(agent: &CodingAgent, path: &Path) -> Result<String, String> {
    let guard = agent.lock().await;
    let json = guard
        .save_messages()
        .map_err(|e| format!("serialize error: {e}"))?;
    let count = guard.messages().len();
    () = drop(guard);

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        () = fs::create_dir_all(parent).map_err(|e| format!("mkdir error: {e}"))?;
    }

    () = fs::write(path, json).map_err(|e| format!("write error: {e}"))?;

    Ok(format!(
        "(session saved to {}, {count} messages)",
        session_status_path(path, default_session_path()),
    ))
}

/// Auto-save non-empty yoagent history for `--continue` (exit paths).
pub async fn try_auto_save_session(agent: &CodingAgent, cwd: &Path) -> Result<(), String> {
    let guard = match tokio::time::timeout(AGENT_LOCK_TIMEOUT, agent.lock()).await {
        Ok(guard) => guard,
        Err(_) => {
            return Err(String::from(
                "agent busy (lock timeout) — auto-save skipped",
            ));
        }
    };
    if guard.messages().is_empty() {
        return Ok(());
    }
    let json = guard
        .save_messages()
        .map_err(|e| format!("serialize error: {e}"))?;
    () = drop(guard);

    let path = last_session_path(cwd);
    if let Some(parent) = path.parent() {
        () = fs::create_dir_all(parent).map_err(|e| format!("mkdir error: {e}"))?;
    }
    () = fs::write(&path, json).map_err(|e| format!("write error: {e}"))?;
    Ok(())
}

/// Restore yoagent messages from JSON into a fresh agent (shared by `/load` and `/jump`).
pub async fn load_agent_from_json(
    agent_config: &AgentConfig,
    json: &str,
) -> Result<(CodingAgent, Vec<AgentMessage>), String> {
    let agent = CodingAgent::new_with_agent_config(agent_config).await;
    {
        let mut guard = agent.lock().await;
        () = guard
            .restore_messages(json)
            .map_err(|e| format!("parse error: {e}"))?;
    }
    let messages = agent.lock().await.messages().to_vec();
    Ok((agent, messages))
}

/// Restore yoagent messages into a fresh agent (reinstall path for `/load` and `--continue`).
pub async fn load_agent_from_file(
    agent_config: &AgentConfig,
    path: &Path,
) -> Result<(CodingAgent, String, Vec<AgentMessage>), String> {
    let json = fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let (agent, messages) = load_agent_from_json(agent_config, &json).await?;
    let count = messages.len();
    Ok((agent, load_status_line(path, count), messages))
}

fn jump_status_line(name: &str, count: usize) -> String {
    format!("✓ jumped to bookmark '{name}' ({count} messages)")
}

/// Restore bookmark JSON into a fresh agent (reinstall path for `/jump`).
pub async fn load_agent_from_bookmark(
    agent_config: &AgentConfig,
    json: &str,
    name: &str,
) -> Result<(CodingAgent, String, Vec<AgentMessage>), String> {
    let (agent, messages) = load_agent_from_json(agent_config, json).await?;
    let count = messages.len();
    Ok((agent, jump_status_line(name, count), messages))
}

pub(super) async fn compact_agent_with_keep(
    agent: &CodingAgent,
    keep_recent: Option<usize>,
) -> Result<String, String> {
    let guard = agent.lock().await;
    let messages = guard.messages().to_vec();
    let before_count = messages.len();
    let before_tokens = total_tokens(&messages) as u64;
    () = drop(guard);

    if before_count == 0 {
        return Ok(nothing_to_compact_message(0, 0));
    }

    let config = compact_context_config(keep_recent);
    let compacted = compact_messages(messages, &config);
    let after_count = compacted.len();
    let after_tokens = total_tokens(&compacted) as u64;

    if before_count == after_count && before_tokens == after_tokens {
        return Ok(nothing_to_compact_message(before_count, before_tokens));
    }

    {
        () = agent.lock().await.replace_messages(compacted);
    }

    Ok(compact_summary(
        before_count,
        before_tokens,
        after_count,
        after_tokens,
    ))
}

/// Dry-run compaction stats without mutating yoagent messages.
pub(super) async fn preview_compact(agent: &CodingAgent, keep_recent: Option<usize>) -> String {
    let guard = agent.lock().await;
    let messages = guard.messages().to_vec();
    let before_count = messages.len();
    let before_tokens = total_tokens(&messages) as u64;
    () = drop(guard);

    if before_count == 0 {
        return nothing_to_compact_message(0, 0);
    }

    let config = compact_context_config(keep_recent);
    let compacted = task::spawn_blocking(move || compact_messages(messages, &config))
        .await
        .expect("compact preview worker");
    let after_count = compacted.len();
    let after_tokens = total_tokens(&compacted) as u64;
    preview_lines(before_count, before_tokens, after_count, after_tokens)
}
