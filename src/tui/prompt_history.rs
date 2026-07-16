//! Grok-aligned prompt history **browse** (empty `↑` recall).
//!
//! Pure helpers + ephemeral browse state — not Session ECS, not Reflect.
//! Data: Session ECS user prompts + shared [`crate::repl::history::ReplInputHistory`].

use std::collections::HashSet;

/// Ephemeral Grok browse panel state (composer live-fill).
///
/// `entries` are **newest-first** (`entries[0]` = most recent).
/// `selected` indexes into `entries` (0 = newest).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptHistoryBrowse {
    pub open: bool,
    pub entries: Vec<String>,
    pub selected: usize,
    /// Composer text when browse opened (usually empty).
    pub saved_draft: String,
}

impl PromptHistoryBrowse {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn selected_text(&self) -> Option<&str> {
        if !self.open {
            return None;
        }
        self.entries.get(self.selected).map(String::as_str)
    }
}

/// Combine Session ECS user prompts (newest-first) with disk bag (oldest-first).
/// Result is newest-first, deduped by `trim()`, first wins (session before disk).
pub fn combine_prompt_history(
    ecs_user_newest_first: &[String],
    disk_oldest_first: &[String],
) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for text in ecs_user_newest_first {
        let key = text.trim().to_string();
        if key.is_empty() {
            continue;
        }
        if seen.insert(key) {
            out.push(text.clone());
        }
    }

    for text in disk_oldest_first.iter().rev() {
        let key = text.trim().to_string();
        if key.is_empty() {
            continue;
        }
        if seen.insert(key) {
            out.push(text.clone());
        }
    }

    out
}

/// Open browse with a pre-built newest-first list. Returns `None` if empty.
pub fn open_browse(entries: Vec<String>, saved_draft: String) -> Option<PromptHistoryBrowse> {
    if entries.is_empty() {
        return None;
    }
    Some(PromptHistoryBrowse {
        open: true,
        entries,
        selected: 0,
        saved_draft,
    })
}

/// Result of a ↓ step while browse is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowseDownResult {
    /// Still open; composer should show this fill.
    StillOpen { fill: String },
    /// Closed (was on newest); restore this draft.
    Closed { restore: String },
}

/// Move to an older entry (↑). No-op at oldest. Returns text to live-fill.
pub fn step_up(browse: &mut PromptHistoryBrowse) -> Option<String> {
    if !browse.open || browse.entries.is_empty() {
        return None;
    }
    let last = browse.entries.len() - 1;
    if browse.selected < last {
        browse.selected += 1;
    }
    browse.selected_text().map(str::to_owned)
}

/// Move toward newer entry (↓). On newest → close and restore saved.
pub fn step_down(browse: &mut PromptHistoryBrowse) -> Option<BrowseDownResult> {
    if !browse.open || browse.entries.is_empty() {
        return None;
    }
    if browse.selected > 0 {
        browse.selected -= 1;
        let fill = browse.selected_text()?.to_owned();
        return Some(BrowseDownResult::StillOpen { fill });
    }
    let restore = close_restore(browse);
    Some(BrowseDownResult::Closed { restore })
}

/// Close browse and return pre-open draft.
pub fn close_restore(browse: &mut PromptHistoryBrowse) -> String {
    let saved = std::mem::take(&mut browse.saved_draft);
    browse.open = false;
    () = browse.entries.clear();
    browse.selected = 0;
    saved
}

/// Accept selection: close browse, return selected text (or saved if none).
pub fn accept(browse: &mut PromptHistoryBrowse) -> String {
    let text = browse
        .selected_text()
        .map(str::to_owned)
        .unwrap_or_else(|| browse.saved_draft.clone());
    browse.open = false;
    () = browse.entries.clear();
    browse.selected = 0;
    () = browse.saved_draft.clear();
    text
}

/// Detach without restoring: close panel, leave composer as-is (caller keeps fill).
pub fn detach(browse: &mut PromptHistoryBrowse) {
    browse.open = false;
    () = browse.entries.clear();
    browse.selected = 0;
    () = browse.saved_draft.clear();
}

/// Collect user-facing prompt texts from Session ECS blocks, **newest first**.
pub fn user_prompt_texts_from_blocks<'a, I>(blocks: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a crate::session::ContentBlock>,
{
    let mut rows: Vec<(u64, u64, String)> = Vec::new();
    for b in blocks {
        let is_user = b.provenance_kind == "userMessage" || b.content_kind == "userMessage";
        if !is_user {
            continue;
        }
        let text = b.content_text.trim();
        if text.is_empty() {
            continue;
        }
        () = rows.push((b.turn_seq, b.seq, b.content_text.clone()));
    }
    () = rows.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    rows.into_iter().map(|(_, _, t)| t).collect()
}
