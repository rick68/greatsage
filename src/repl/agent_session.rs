use {
    crate::agents::CodingAgent,
    super::compact_parse::CompactArg,
    std::path::{Path, PathBuf},
    yoagent::context::{compact_messages, total_tokens, ContextConfig},
};

pub const DEFAULT_SESSION_PATH: &str = "greatsage-session.json";

pub fn default_session_path() -> PathBuf {
    PathBuf::from(DEFAULT_SESSION_PATH)
}

pub fn resolve_session_path(path: Option<&str>) -> PathBuf {
    path.filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_session_path)
}

pub async fn save_messages(agent: &CodingAgent, path: &Path) -> Result<String, String> {
    let guard = agent.lock().await;
    let json = guard
        .save_messages()
        .map_err(|e| format!("serialize error: {e}"))?;
    let count = guard.messages().len();
    drop(guard);
    std::fs::write(path, json).map_err(|e| format!("write error: {e}"))?;
    Ok(format!(
        "Session saved to {} ({} messages).",
        path.display(),
        count
    ))
}

pub async fn load_messages(agent: &CodingAgent, path: &Path) -> Result<String, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
    let mut guard = agent.lock().await;
    guard
        .restore_messages(&json)
        .map_err(|e| format!("parse error: {e}"))?;
    let count = guard.messages().len();
    Ok(format!(
        "Session loaded from {} ({} messages).",
        path.display(),
        count
    ))
}

pub async fn compact_agent(agent: &CodingAgent, arg: CompactArg) -> Result<String, String> {
    match arg {
        CompactArg::Invalid => {
            return Err("invalid argument — use a number, \"all\", or \"--preview\"".to_string());
        }
        CompactArg::Preview => return Ok(compact_preview(agent).await),
        CompactArg::Default | CompactArg::KeepRecent(_) => {}
    }

    let keep_recent = match arg {
        CompactArg::KeepRecent(n) => Some(n),
        CompactArg::Default => None,
        _ => unreachable!(),
    };

    let guard = agent.lock().await;
    let messages = guard.messages().to_vec();
    let before_count = messages.len();
    let before_tokens = total_tokens(&messages) as u64;
    drop(guard);

    if before_count == 0 {
        return Ok("Nothing to compact (empty conversation).".to_string());
    }

    let config = match keep_recent {
        Some(kr) => ContextConfig {
            max_context_tokens: 0,
            system_prompt_tokens: 0,
            keep_recent: kr,
            ..ContextConfig::default()
        },
        None => ContextConfig::default(),
    };

    let compacted = compact_messages(messages, &config);
    let after_count = compacted.len();
    let after_tokens = total_tokens(&compacted) as u64;

    if before_count == after_count && before_tokens == after_tokens {
        return Ok(format!(
            "No compaction needed ({before_count} messages, {before_tokens} tokens)."
        ));
    }

    let mut guard = agent.lock().await;
    guard.replace_messages(compacted);
    drop(guard);

    let keep_label = keep_recent
        .map(|n| format!(" (kept last {n})"))
        .unwrap_or_default();

    Ok(format!(
        "Compacted{keep_label}: {before_count} → {after_count} messages, {before_tokens} → {after_tokens} tokens."
    ))
}

async fn compact_preview(agent: &CodingAgent) -> String {
    let guard = agent.lock().await;
    let messages = guard.messages();
    let msg_count = messages.len();
    let current_tokens = total_tokens(messages) as u64;
    drop(guard);

    if msg_count == 0 {
        return "Compact preview: nothing to compact (empty conversation).".to_string();
    }

    let keep_recent = ContextConfig::default().keep_recent;
    let would_keep = msg_count.min(keep_recent);
    let would_compress = msg_count.saturating_sub(keep_recent);

    format!(
        "Compact preview: {msg_count} messages ({current_tokens} tokens); \
         default would keep ~{would_keep} at full fidelity and compress ~{would_compress} older messages."
    )
}