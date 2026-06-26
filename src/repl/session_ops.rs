//! yoagent save / load / compact backends for session commands.
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.

use {
    super::path_display::session_status_path,
    crate::agents::CodingAgent,
    std::{
        env, fs,
        path::{Path, PathBuf},
    },
    yoagent::context::{ContextConfig, compact_messages, total_tokens},
};

pub const DEFAULT_SESSION_FILENAME: &str = "greatsage-session.json";

fn default_session_path() -> PathBuf {
    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(DEFAULT_SESSION_FILENAME)
}

/// Blocking snapshot of yoagent message stats for `/clear` confirmation.
pub(super) fn agent_message_stats_blocking(agent: &CodingAgent) -> (usize, u64) {
    let agent = agent.clone();
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Runtime::new()
                    .expect("clear stats runtime")
                    .block_on(async {
                        let guard = agent.lock().await;
                        let messages = guard.messages();
                        (messages.len(), total_tokens(messages) as u64)
                    })
            })
            .join()
            .expect("clear stats thread")
    })
}

fn nothing_to_compact_message(message_count: usize, token_count: u64) -> String {
    format!("(nothing to compact — {message_count} messages, ~{token_count} tokens)")
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

pub(super) async fn load_messages(agent: &CodingAgent, path: &Path) -> Result<String, String> {
    let json = fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let mut guard = agent.lock().await;

    () = guard
        .restore_messages(&json)
        .map_err(|e| format!("parse error: {e}"))?;

    let count = guard.messages().len();

    Ok(format!(
        "(session loaded from {}, {count} messages)",
        session_status_path(path, default_session_path()),
    ))
}

pub(super) async fn compact_agent(agent: &CodingAgent) -> Result<String, String> {
    let guard = agent.lock().await;
    let messages = guard.messages().to_vec();
    let before_count = messages.len();
    let before_tokens = total_tokens(&messages) as u64;
    () = drop(guard);

    if before_count == 0 {
        return Ok(nothing_to_compact_message(0, 0));
    }

    let compacted = compact_messages(messages, &ContextConfig::default());
    let after_count = compacted.len();
    let after_tokens = total_tokens(&compacted) as u64;

    if before_count == after_count && before_tokens == after_tokens {
        return Ok(nothing_to_compact_message(before_count, before_tokens));
    }

    {
        () = agent.lock().await.replace_messages(compacted);
    }

    Ok(format!(
        "Compacted: {before_count} → {after_count} messages, {before_tokens} → {after_tokens} tokens."
    ))
}
