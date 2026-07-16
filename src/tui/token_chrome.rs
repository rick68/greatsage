//! Compact Session-ECS token/cost usage for idle TUI status chrome.
//!
//! Token numbers come from `SessionContextStats` + `SessionLifetimeUsage` (same truth as
//! `/tokens` / BRP). Estimated dollars reuse `repl::cost` (`estimate_cost` / `format_cost`)
//! over lifetime usage × current model rates — no private counter or parallel ledger.

use {
    crate::{
        providers::Provider,
        repl::{
            cost::{estimate_cost, format_cost},
            session_dashboard::format_token_amount,
        },
    },
    bevy::utils::default,
    yoagent::types::Usage,
};

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

    /// Convert to yoagent `Usage` for shared cost helpers.
    pub fn to_usage(self) -> Usage {
        Usage {
            input: self.input,
            output: self.output,
            cache_read: self.cache_read,
            cache_write: self.cache_write,
            ..default()
        }
    }
}

/// Build compact usage fragment: context fill first, then lifetime totals.
///
/// - `context_max == 0` → unknown window; show used only when `context_used > 0`
/// - all-zero lifetime → omit totals
/// - both empty → `None` (caller keeps key-hint chrome only)
///
/// Examples: `ctx:12.0k/256.0k (5%) · Σ1.2M`, `ctx:933`, `Σ4.5k`
///
/// Token-only — no pricing deps. Cost is a separate fragment.
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

/// Compact estimated session cost for idle status (`$0.0016` style).
///
/// - zero lifetime → `None`
/// - unpriced model (`estimate_cost` unavailable) → `None`
/// - otherwise `format_cost` of the estimated total
pub fn format_cost_status_fragment(
    lifetime: LifetimeTokens,
    provider: Provider,
    model: &str,
) -> Option<String> {
    if lifetime.is_zero() {
        return None;
    }
    let usage = lifetime.to_usage();
    let total = estimate_cost(&usage, provider, model)?;
    Some(format_cost(total))
}

/// Dim separator between status segments.
pub const STATUS_SEGMENT_SEP: &str = " · ";

/// Idle status segment (display order left→right).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusSegment {
    Hints(String),
    Auth(String),
    Usage(String),
    Cost(String),
}

impl StatusSegment {
    pub fn text(&self) -> &str {
        match self {
            Self::Hints(s) | Self::Auth(s) | Self::Usage(s) | Self::Cost(s) => s.as_str(),
        }
    }

    /// Lower value = drop first when width is tight (design D4).
    fn elide_rank(&self) -> u8 {
        match self {
            Self::Hints(_) => 0,
            Self::Auth(_) => 1,
            Self::Usage(_) => 2,
            Self::Cost(_) => 3,
        }
    }
}

/// Laid-out status for 1 or 2 rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusLayout {
    pub lines: Vec<String>,
}

impl StatusLayout {
    pub fn status_rows(&self) -> u16 {
        self.lines.len().clamp(1, 2) as u16
    }

    #[allow(dead_code)] // used by `compose_status_line` + tests
    pub fn primary_line(&self) -> &str {
        self.lines.first().map(String::as_str).unwrap_or("")
    }
}

/// Build idle segments in display order (skips empty optional pieces).
pub fn build_idle_segments(
    default_hint: &str,
    auth_chrome: Option<&str>,
    usage_fragment: Option<&str>,
    cost_fragment: Option<&str>,
) -> Vec<StatusSegment> {
    let mut out = Vec::with_capacity(4);
    if !default_hint.is_empty() {
        () = out.push(StatusSegment::Hints(default_hint.to_owned()));
    }
    if let Some(auth) = auth_chrome.filter(|s| !s.is_empty()) {
        () = out.push(StatusSegment::Auth(auth.to_owned()));
    }
    if let Some(usage) = usage_fragment.filter(|s| !s.is_empty()) {
        () = out.push(StatusSegment::Usage(usage.to_owned()));
    }
    if let Some(cost) = cost_fragment.filter(|s| !s.is_empty()) {
        () = out.push(StatusSegment::Cost(cost.to_owned()));
    }
    out
}

/// Join segments with [`STATUS_SEGMENT_SEP`].
pub fn join_status_segments(segments: &[StatusSegment]) -> String {
    segments
        .iter()
        .map(StatusSegment::text)
        .collect::<Vec<_>>()
        .join(STATUS_SEGMENT_SEP)
}

/// Approximate display width (ASCII-oriented; status chrome is Latin-heavy).
fn approx_width(s: &str) -> usize {
    s.chars().count()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max {
        return s.to_owned();
    }
    s.chars().take(max).collect()
}

/// Drop lowest-priority segments until `join` fits `width`, then hard-truncate.
fn elide_to_width(mut segments: Vec<StatusSegment>, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    loop {
        let joined = join_status_segments(&segments);
        if approx_width(&joined) <= width {
            return joined;
        }
        // Drop lowest elide_rank first (hints → auth → usage); keep cost longest.
        let drop_idx = segments
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| s.elide_rank())
            .map(|(i, _)| i);
        match drop_idx {
            Some(i) if segments.len() > 1 => {
                let _ = segments.remove(i);
            }
            _ => {
                return truncate_chars(&joined, width);
            }
        }
    }
}

/// Fit idle segments into 1 row, or 2 when `allow_two_rows` and needed.
///
/// Two-row split: line 0 = key hints (when present); line 1 = remaining
/// segments (auth · usage · cost), each elided to `width`.
pub fn fit_status_segments(
    segments: &[StatusSegment],
    width: u16,
    allow_two_rows: bool,
) -> StatusLayout {
    let w = width.max(1) as usize;
    if segments.is_empty() {
        return StatusLayout {
            lines: vec![String::new()],
        };
    }

    let one = elide_to_width(segments.to_vec(), w);
    if approx_width(&join_status_segments(segments)) <= w {
        return StatusLayout { lines: vec![one] };
    }

    if allow_two_rows {
        let (hints, rest): (Vec<_>, Vec<_>) = segments
            .iter()
            .cloned()
            .partition(|s| matches!(s, StatusSegment::Hints(_)));
        if !hints.is_empty() && !rest.is_empty() {
            let line0 = elide_to_width(hints, w);
            let line1 = elide_to_width(rest, w);
            return StatusLayout {
                lines: vec![line0, line1],
            };
        }
    }

    StatusLayout { lines: vec![one] }
}

/// Compose status hierarchy with sticky override and optional 2-row fit.
///
/// Sticky `status_hint` wins entirely (no forced usage/cost embed) — single row.
/// Idle path: segments `hints · auth · usage · cost` with priority elision.
pub fn compose_status_hierarchy(
    status_hint: Option<&str>,
    default_hint: &str,
    auth_chrome: Option<&str>,
    usage_fragment: Option<&str>,
    cost_fragment: Option<&str>,
    width: u16,
    allow_two_rows: bool,
) -> StatusLayout {
    if let Some(hint) = status_hint {
        let w = width.max(1) as usize;
        return StatusLayout {
            lines: vec![truncate_chars(hint, w)],
        };
    }
    let segments = build_idle_segments(default_hint, auth_chrome, usage_fragment, cost_fragment);
    fit_status_segments(&segments, width, allow_two_rows)
}

/// Compose the idle status line with optional sticky override (single string).
///
/// Sticky `status_hint` wins entirely (no forced usage/cost embed). Idle path:
/// `default_hint · auth · usage · cost` (each optional when `None`/empty).
///
/// Prefer [`compose_status_hierarchy`] when width-aware layout is available.
#[allow(dead_code)] // kept for callers/tests that want a single joined string
pub fn compose_status_line(
    status_hint: Option<&str>,
    default_hint: &str,
    auth_chrome: Option<&str>,
    usage_fragment: Option<&str>,
    cost_fragment: Option<&str>,
) -> String {
    compose_status_hierarchy(
        status_hint,
        default_hint,
        auth_chrome,
        usage_fragment,
        cost_fragment,
        u16::MAX,
        false,
    )
    .primary_line()
    .to_owned()
}
