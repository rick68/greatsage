//! Compact Session-ECS token usage for idle TUI status chrome.
//!
//! Numbers come from `SessionContextStats` + `SessionLifetimeUsage` (same truth as
//! `/tokens` / BRP). Formatting reuses dashboard helpers — no private counter.

use crate::repl::session_dashboard::format_token_amount;

/// Lifetime field bag for pure formatting (mirrors `SessionLifetimeUsage` / `Usage`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LifetimeTokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl LifetimeTokens {
    pub fn total(self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }

    pub fn is_zero(self) -> bool {
        self.total() == 0
    }
}

/// Build compact usage fragment: context fill first, then lifetime totals.
///
/// - `context_max == 0` → unknown window; show used only when `context_used > 0`
/// - all-zero lifetime → omit totals
/// - both empty → `None` (caller keeps key-hint chrome only)
///
/// Examples: `ctx:12.0k/256.0k (5%) · Σ1.2M`, `ctx:933`, `Σ4.5k`
pub fn format_usage_status_fragment(
    context_used: u64,
    context_max: u64,
    lifetime: LifetimeTokens,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::with_capacity(2);

    if let Some(ctx) = format_context_fragment(context_used, context_max) {
        parts.push(ctx);
    }
    if let Some(tot) = format_lifetime_fragment(lifetime) {
        parts.push(tot);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

fn format_context_fragment(used: u64, max: u64) -> Option<String> {
    if max > 0 {
        let pct = (used as f64 / max as f64 * 100.0).min(100.0);
        Some(format!(
            "ctx:{}/{} ({:.0}%)",
            format_token_amount(used),
            format_token_amount(max),
            pct
        ))
    } else if used > 0 {
        Some(format!("ctx:{}", format_token_amount(used)))
    } else {
        None
    }
}

fn format_lifetime_fragment(lifetime: LifetimeTokens) -> Option<String> {
    if lifetime.is_zero() {
        None
    } else {
        Some(format!("Σ{}", format_token_amount(lifetime.total())))
    }
}

/// Compose the idle status line with optional sticky override.
///
/// Sticky `status_hint` wins entirely (no forced usage embed). Idle path:
/// `default_hint · auth · usage` (auth/usage omitted when `None`).
pub fn compose_status_line(
    status_hint: Option<&str>,
    default_hint: &str,
    auth_chrome: Option<&str>,
    usage_fragment: Option<&str>,
) -> String {
    if let Some(hint) = status_hint {
        return hint.to_owned();
    }

    let mut line = default_hint.to_owned();
    if let Some(auth) = auth_chrome.filter(|s| !s.is_empty()) {
        line = format!("{line} · {auth}");
    }
    if let Some(usage) = usage_fragment.filter(|s| !s.is_empty()) {
        line = format!("{line} · {usage}");
    }
    line
}
