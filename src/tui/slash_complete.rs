//! Slash autocomplete helpers for TUI (shared engine: `repl::completion`).

use {
    super::state::{SlashMenuState, TuiFocus, TuiState},
    crate::{
        agents::AgentConfig,
        repl::{
            apply_replacement, common_prefix, completions, inline_hint, token_bounds, token_prefix,
        },
    },
};

/// Max candidate rows drawn in the TUI menu.
pub const SLASH_MENU_MAX_ROWS: usize = 8;

/// Result of Tab while prompt-focused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabSlashResult {
    /// Accepted or expanded a slash candidate (stay on prompt).
    Completing,
    /// Not slash-completing — caller should toggle focus to scrollback.
    FocusScrollback,
}

fn cursor_char_index(prompt: &str, cursor_byte: usize) -> usize {
    let byte = cursor_byte.min(prompt.len());
    prompt[..byte].chars().count()
}

fn char_index_to_byte(prompt: &str, char_idx: usize) -> usize {
    prompt
        .char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(prompt.len())
}

/// Close menu and drop cached candidates (ghost cleared separately by refresh).
pub fn close_slash_menu(state: &mut TuiState) {
    state.slash_menu = SlashMenuState::default();
}

/// Recompute ghost + candidates. Opens menu when eligible (not bare `/` unless `force_open`).
pub fn refresh_slash_completion(
    state: &mut TuiState,
    agent_config: &AgentConfig,
    force_open: bool,
) {
    let prompt = state.prompt.as_str();
    if !prompt.starts_with('/') {
        state.ghost_hint = None;
        close_slash_menu(state);
        return;
    }

    let cursor_chars = cursor_char_index(prompt, state.cursor);
    let at_end = cursor_chars == prompt.chars().count();
    state.ghost_hint = if at_end {
        inline_hint(prompt, cursor_chars, agent_config)
    } else {
        None
    };

    let candidates = completions(prompt, cursor_chars, agent_config);
    if candidates.is_empty() {
        () = close_slash_menu(state);
        return;
    }

    // Sole exact token match (e.g. draft `/status`, candidates `["/status"]`):
    // keep ghost, **close menu** so the next Enter dispatches/submits instead of
    // re-accepting forever. Args still incomplete (`/model ` → models) keep the menu.
    let token = token_prefix(prompt, cursor_chars);
    if candidates.len() == 1 && candidates[0] == token {
        () = close_slash_menu(state);
        return;
    }

    // Grok slash menu: typing `/` opens the command list immediately.
    let prev = state.slash_menu.highlight;
    let highlight = prev.min(candidates.len().saturating_sub(1));
    let _ = force_open; // retained for Tab-force API; bare `/` now always opens
    state.slash_menu = SlashMenuState {
        open: true,
        highlight,
        candidates,
    };
}

/// Accept the highlighted candidate into the draft; refresh afterward.
/// Returns `true` if a candidate was applied.
pub fn accept_slash_candidate(state: &mut TuiState, agent_config: &AgentConfig) -> bool {
    if !state.slash_menu.open || state.slash_menu.candidates.is_empty() {
        return false;
    }
    let idx = state
        .slash_menu
        .highlight
        .min(state.slash_menu.candidates.len() - 1);
    let replacement = state.slash_menu.candidates[idx].clone();
    () = apply_token_to_prompt(state, &replacement);
    // After accept, re-open only if further completions remain (args etc.).
    () = refresh_slash_completion(state, agent_config, false);
    true
}

fn apply_token_to_prompt(state: &mut TuiState, replacement: &str) {
    let cursor_chars = cursor_char_index(&state.prompt, state.cursor);
    let (start, end) = token_bounds(&state.prompt, cursor_chars);
    state.prompt = apply_replacement(&state.prompt, start, end, replacement);
    let new_end_chars = start + replacement.chars().count();
    state.cursor = char_index_to_byte(&state.prompt, new_end_chars);
}

/// Tab on prompt: complete when slash-active; otherwise focus scrollback.
pub fn handle_prompt_tab(state: &mut TuiState, agent_config: &AgentConfig) -> TabSlashResult {
    if !state.prompt.starts_with('/') {
        () = close_slash_menu(state);
        state.ghost_hint = None;
        return TabSlashResult::FocusScrollback;
    }

    // Menu open → accept highlight.
    if state.slash_menu.open && !state.slash_menu.candidates.is_empty() {
        let _ = accept_slash_candidate(state, agent_config);
        return TabSlashResult::Completing;
    }

    let cursor_chars = cursor_char_index(&state.prompt, state.cursor);
    let candidates = completions(&state.prompt, cursor_chars, agent_config);

    if candidates.is_empty() {
        // Slash line but no hits → still "completing" context? Spec: non-slash focus toggle.
        // Bare unknown `/xyz` with no candidates: fall through to focus toggle is OK.
        () = close_slash_menu(state);
        state.ghost_hint = None;
        return TabSlashResult::FocusScrollback;
    }

    if candidates.len() == 1 {
        () = apply_token_to_prompt(state, &candidates[0]);
        () = refresh_slash_completion(state, agent_config, false);
        return TabSlashResult::Completing;
    }

    // Multi: expand common prefix if longer than token, then open menu.
    let prefix = {
        let (start, end) = token_bounds(&state.prompt, cursor_chars);
        state
            .prompt
            .chars()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect::<String>()
    };
    let shared = common_prefix(&candidates);
    if shared.len() > prefix.len() && shared.starts_with(&prefix) {
        () = apply_token_to_prompt(state, &shared);
    }
    // Force open menu (including bare `/`).
    () = refresh_slash_completion(state, agent_config, true);
    TabSlashResult::Completing
}

/// Move menu highlight by `delta` (±1); wraps.
pub fn move_menu_highlight(state: &mut TuiState, delta: i32) {
    if !state.slash_menu.open || state.slash_menu.candidates.is_empty() {
        return;
    }
    let n = state.slash_menu.candidates.len() as i32;
    let cur = state.slash_menu.highlight as i32;
    let next = (cur + delta).rem_euclid(n) as usize;
    state.slash_menu.highlight = next;
}

/// Esc with menu open: dismiss only. Returns `true` if consumed.
pub fn try_dismiss_slash_menu(state: &mut TuiState) -> bool {
    if state.slash_menu.open {
        () = close_slash_menu(state);
        // Keep ghost if still slash; refresh without force (may stay closed on bare `/`)
        // Caller supplies AgentConfig for full refresh; here only dismiss menu.
        state.ghost_hint = None;
        return true;
    }
    false
}

/// Whether prompt Enter should accept menu instead of submit/dispatch.
pub fn enter_should_accept_menu(state: &TuiState) -> bool {
    state.focus == TuiFocus::Prompt
        && state.slash_menu.open
        && !state.slash_menu.candidates.is_empty()
}
