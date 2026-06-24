use yoagent::types::Usage;

pub(crate) fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

pub(crate) fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Provider-reported `Usage.input` is uncached prompt tokens only; cache hits live in
/// `cache_read` / `cache_write`. Show total input so REPL hints match billed context size.
pub(crate) fn format_usage_line(usage: &Usage) -> Option<String> {
    let Usage {
        input,
        output,
        cache_read,
        cache_write,
        ..
    } = usage;
    let total_in = input
        .saturating_add(*cache_read)
        .saturating_add(*cache_write);
    if total_in == 0 && *output == 0 {
        return None;
    }

    let mut line = format!("tokens: {total_in} in / {output} out");
    if *cache_read > 0 || *cache_write > 0 {
        let mut parts = Vec::new();
        if *input > 0 {
            parts.push(format!("{input} new"));
        }
        if *cache_read > 0 {
            parts.push(format!("{cache_read} cache read"));
        }
        if *cache_write > 0 {
            parts.push(format!("{cache_write} cache write"));
        }
        line.push_str(&format!(" ({})", parts.join(", ")));
    }
    Some(line)
}
