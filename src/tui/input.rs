//! Keyboard + mouse input via `bevy_ratatui` messages.

use {
    super::{
        commands::is_ui_quit_line,
        nav::{jump_end, jump_home, jump_turn, move_selection_by, ratatui_scroll_y, scroll_page},
        scrollback::ScrollbackView,
        slash_complete::{
            TabSlashResult, enter_should_accept_menu, handle_prompt_tab, move_menu_highlight,
            refresh_slash_completion, try_dismiss_slash_menu,
        },
        state::{ESC_CLEAR_WINDOW, TuiFocus, TuiState, rect_contains},
    },
    crate::{
        agents::{AgentConfig, CodingAgent, CodingAgentPromptChannel},
        cli::Cli,
        config::Config,
        repl::{DispatchResult, dispatch_slash_command, session_state::ReplSessionState},
        session::SessionRuntimeStatus,
    },
    bevy::{
        app::AppExit,
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageReader, MessageWriter},
            system::Query,
        },
    },
    bevy_ratatui::{
        crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
        event::{KeyMessage, MouseMessage, PasteMessage},
    },
    bevy_tokio_tasks::TokioTasksRuntime,
    std::time::Instant,
};

/// Live-stream body + finish footer for `/run` / `!` (shared path with line REPL).
pub fn poll_shell_system(mut session: ResMut<ReplSessionState>, mut state: ResMut<TuiState>) {
    let Some(handle) = session.active_shell.as_mut() else {
        return;
    };

    while let Some(live) = crate::repl::shell_run::try_recv_shell_live(handle) {
        handle.body_streamed = true;
        let line = crate::repl::shell_run::format_live_stream_line(&live);
        let display = strip_shell_prefix(&line);
        () = state.push_operator_line(display.to_string());
    }

    let Some(result) = crate::repl::shell_run::try_recv_shell_result(handle) else {
        return;
    };
    while let Some(live) = crate::repl::shell_run::try_recv_shell_live(handle) {
        handle.body_streamed = true;
        let line = crate::repl::shell_run::format_live_stream_line(&live);
        let display = strip_shell_prefix(&line);
        () = state.push_operator_line(display.to_string());
    }
    let body_streamed = handle.body_streamed;
    session.active_shell = None;
    if result.success {
        session.last_failed_run = None;
    } else {
        session.last_failed_run = Some(result.clone());
    }
    let (output, detail) =
        crate::repl::shell_run::format_run_output_lines_ex(&result, body_streamed);
    for line in output.into_iter().chain(detail) {
        let display = strip_shell_prefix(&line);
        () = state.push_operator_line(display.to_string());
    }
}

fn strip_shell_prefix(line: &str) -> &str {
    line.strip_prefix(crate::repl::shell_run::RUN_STDERR_BODY_PREFIX)
        .or_else(|| line.strip_prefix(crate::repl::shell_run::RUN_STDIN_EOF_BODY_PREFIX))
        .unwrap_or(line)
}

/// Mouse: left-click focuses panes; wheel scrolls panel or scrollback.
pub fn mouse_input_system(
    mut mice: MessageReader<MouseMessage>,
    mut state: ResMut<TuiState>,
    scrollback: Res<ScrollbackView>,
) {
    let line_count = scrollback.line_count();
    for message in mice.read() {
        let ev = message.0;
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if state.operator_panel.open
                    && rect_contains(state.operator_panel.last_rect, ev.column, ev.row)
                {
                    let rect = state.operator_panel.last_rect;
                    if rect.width > 2 && rect.height > 2 && !state.operator_panel.lines.is_empty() {
                        let inner_row = ev.row.saturating_sub(rect.y.saturating_add(1));
                        let n = state.operator_panel.lines.len();
                        let hi = state.operator_panel.selected.min(n.saturating_sub(1));
                        let max_vis = (rect.height.saturating_sub(2) as usize).max(1);
                        let start = if n <= max_vis {
                            0
                        } else {
                            hi.saturating_sub(max_vis / 2).min(n - max_vis)
                        };
                        let idx = start.saturating_add(inner_row as usize).min(n - 1);
                        state.operator_panel.selected = idx;
                        state.sync_prompt_from_operator_selection();
                    }
                    continue;
                }
                if rect_contains(state.last_prompt_rect, ev.column, ev.row) {
                    state.focus = TuiFocus::Prompt;
                    state.clear_esc_arm();
                } else if rect_contains(state.last_scrollback_rect, ev.column, ev.row) {
                    state.focus = TuiFocus::Scrollback;
                    state.clear_esc_arm();
                    let rect = state.last_scrollback_rect;
                    if rect.width > 2 && rect.height > 2 && line_count > 0 {
                        let inner_row = ev.row.saturating_sub(rect.y.saturating_add(1));
                        let inner_h = rect.height.saturating_sub(2).max(1);
                        let top = ratatui_scroll_y(line_count, inner_h, state.scroll_from_bottom)
                            as usize;
                        let idx = top.saturating_add(inner_row as usize);
                        state.selected_line = idx.min(line_count - 1);
                    }
                }
            }
            // Wheel: panel steals when open; else scrollback.
            MouseEventKind::ScrollUp => {
                if state.operator_panel.open {
                    state.operator_panel.move_selection(1);
                    state.sync_prompt_from_operator_selection();
                } else {
                    () = move_selection_by(&mut state, line_count, 1);
                }
            }
            MouseEventKind::ScrollDown => {
                if state.operator_panel.open {
                    state.operator_panel.move_selection(-1);
                    state.sync_prompt_from_operator_selection();
                } else {
                    () = move_selection_by(&mut state, line_count, -1);
                }
            }
            _ => {}
        }
    }
}

/// Bracketed paste / multi-char IME commit → insert at prompt cursor.
pub fn paste_system(
    mut pastes: MessageReader<PasteMessage>,
    mut state: ResMut<TuiState>,
    agent_config: Res<AgentConfig>,
) {
    for paste in pastes.read() {
        if state.focus != TuiFocus::Prompt || state.operator_panel.open {
            continue;
        }
        state.clear_esc_arm();
        state.clear_ime_preedit();
        insert_str(&mut state, &paste.0);
        refresh_slash_completion(&mut state, &agent_config, false);
    }
}

pub fn input_system(
    mut keys: MessageReader<KeyMessage>,
    mut state: ResMut<TuiState>,
    scrollback: Res<ScrollbackView>,
    mut exit: MessageWriter<AppExit>,
    prompt_channel: Res<CodingAgentPromptChannel>,
    mut agent_config: ResMut<AgentConfig>,
    mut session: ResMut<ReplSessionState>,
    config: Res<Config>,
    cli: Res<Cli>,
    coding_agent: Option<Res<CodingAgent>>,
    tokio_runtime: Res<TokioTasksRuntime>,
    runtime_status: Query<&SessionRuntimeStatus>,
) {
    let line_count = scrollback.line_count();
    let turn_starts = scrollback.turn_starts.clone();
    let agent_busy = runtime_status.iter().any(|s| s.is_processing());

    for message in keys.read() {
        // Ignore Null (some IME/control sequences) and non-press kinds.
        if matches!(message.code, KeyCode::Null) {
            continue;
        }
        if message.kind != KeyEventKind::Press && message.kind != KeyEventKind::Repeat {
            continue;
        }

        // ── Ctrl+C / Ctrl+D (foundation) ──────────────────────────────────
        if message.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(message.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            state.clear_esc_arm();
            if session.ctrl_c_armed {
                if let Some(handle) = session.active_shell.as_mut() {
                    crate::repl::shell_run::request_shell_interrupt(handle);
                    crate::repl::shell_run::request_shell_interrupt(handle);
                }
                session.active_shell = None;
                session.ctrl_c_armed = false;
                exit.write_default();
                return;
            }

            if let Some(handle) = session.active_shell.as_mut() {
                crate::repl::shell_run::request_shell_interrupt(handle);
                session.ctrl_c_armed = true;
                () = state.set_status_hint("^C");
                continue;
            }

            if agent_busy {
                if let Some(agent_res) = coding_agent.as_ref() {
                    let agent: crate::agents::CodingAgent = (**agent_res).clone();
                    tokio_runtime.runtime().spawn(async move {
                        let guard = agent.lock().await;
                        guard.abort();
                    });
                }
                session.ctrl_c_armed = true;
                () = state.set_status_hint("^C");
                continue;
            }

            () = state.clear_prompt();
            session.ctrl_c_armed = true;
            continue;
        }

        if message.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(message.code, KeyCode::Char('d') | KeyCode::Char('D'))
        {
            state.clear_esc_arm();
            if let Some(handle) = session.active_shell.as_ref() {
                crate::repl::shell_run::request_shell_stdin_eof(handle);
                continue;
            }
            session.ctrl_c_armed = false;
            exit.write_default();
            return;
        }

        if session.active_shell.is_some() {
            continue;
        }

        if session.ctrl_c_armed {
            session.ctrl_c_armed = false;
        }

        // ── Esc steal: IME preedit → operator panel → slash menu → clear ──
        if message.code == KeyCode::Esc {
            if !state.ime_preedit.is_empty() {
                // Cancel composition first (do not clear the committed prompt).
                state.clear_ime_preedit();
                continue;
            }
            if state.operator_panel.open {
                state.close_operator_panel();
                continue;
            }
            if try_dismiss_slash_menu(&mut state) {
                refresh_slash_completion(&mut state, &agent_config, false);
                continue;
            }
            handle_esc(
                &mut state,
                agent_busy || session.active_shell.is_some(),
                Instant::now(),
            );
            continue;
        }

        // ── Operator panel navigation (separate window; not scrollback) ───
        if state.operator_panel.open {
            match message.code {
                KeyCode::Up => {
                    state.operator_panel.move_selection(-1);
                    state.sync_prompt_from_operator_selection();
                }
                KeyCode::Down => {
                    state.operator_panel.move_selection(1);
                    state.sync_prompt_from_operator_selection();
                }
                KeyCode::PageUp => {
                    state.operator_panel.move_selection(-10);
                    state.sync_prompt_from_operator_selection();
                }
                KeyCode::PageDown => {
                    state.operator_panel.move_selection(10);
                    state.sync_prompt_from_operator_selection();
                }
                KeyCode::Home => {
                    state.operator_panel.selected = 0;
                    state.sync_prompt_from_operator_selection();
                }
                KeyCode::End => {
                    let n = state.operator_panel.lines.len();
                    state.operator_panel.selected = n.saturating_sub(1);
                    state.sync_prompt_from_operator_selection();
                }
                KeyCode::Enter => {
                    // Fill prompt from selection; close panel; stay ready to run.
                    state.sync_prompt_from_operator_selection();
                    state.close_operator_panel();
                    state.focus = TuiFocus::Prompt;
                    refresh_slash_completion(&mut state, &agent_config, false);
                }
                KeyCode::Tab => {
                    state.focus = TuiFocus::Prompt;
                    state.clear_esc_arm();
                    refresh_slash_completion(&mut state, &agent_config, false);
                }
                _ => {}
            }
            continue;
        }

        // ── Page keys: both foci ──────────────────────────────────────────
        if matches!(message.code, KeyCode::PageUp | KeyCode::PageDown)
            && !message.modifiers.contains(KeyModifiers::SHIFT)
        {
            let up = message.code == KeyCode::PageUp;
            () = scroll_page(&mut state, line_count, up);
            continue;
        }

        // ── Scrollback-focused navigation ─────────────────────────────────
        if state.focus == TuiFocus::Scrollback {
            match message.code {
                KeyCode::Tab => {
                    state.focus = TuiFocus::Prompt;
                    state.clear_esc_arm();
                    refresh_slash_completion(&mut state, &agent_config, false);
                }
                KeyCode::Char(' ')
                    if !message
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    state.focus = TuiFocus::Prompt;
                    state.clear_esc_arm();
                    refresh_slash_completion(&mut state, &agent_config, false);
                }
                KeyCode::Up => move_selection_by(&mut state, line_count, -1),
                KeyCode::Down => move_selection_by(&mut state, line_count, 1),
                KeyCode::Left if message.modifiers.contains(KeyModifiers::SHIFT) => {
                    jump_turn(&mut state, &turn_starts, line_count, false);
                }
                KeyCode::Right if message.modifiers.contains(KeyModifiers::SHIFT) => {
                    jump_turn(&mut state, &turn_starts, line_count, true);
                }
                KeyCode::Home => jump_home(&mut state, line_count),
                KeyCode::End => jump_end(&mut state, line_count),
                KeyCode::Char(c)
                    if !message.modifiers.contains(KeyModifiers::CONTROL)
                        && !message.modifiers.contains(KeyModifiers::ALT)
                        && !c.is_control() =>
                {
                    state.focus = TuiFocus::Prompt;
                    () = state.clear_esc_arm();
                    insert_char(&mut state, c);
                    refresh_slash_completion(&mut state, &agent_config, false);
                }
                _ => {}
            }
            continue;
        }

        // ── Prompt-focused: menu arrows before edit keys ──────────────────
        if state.slash_menu.open {
            match message.code {
                KeyCode::Up => {
                    move_menu_highlight(&mut state, -1);
                    continue;
                }
                KeyCode::Down => {
                    move_menu_highlight(&mut state, 1);
                    continue;
                }
                _ => {}
            }
        }

        // ── Prompt-focused edit ───────────────────────────────────────────
        match message.code {
            KeyCode::Tab => {
                state.clear_esc_arm();
                match handle_prompt_tab(&mut state, &agent_config) {
                    TabSlashResult::Completing => {}
                    TabSlashResult::FocusScrollback => {
                        state.focus = TuiFocus::Scrollback;
                    }
                }
            }
            KeyCode::Char(c)
                if !message.modifiers.contains(KeyModifiers::CONTROL)
                    && !message.modifiers.contains(KeyModifiers::ALT)
                    && !c.is_control() =>
            {
                state.clear_esc_arm();
                // Committed text clears any stale preedit slot.
                state.clear_ime_preedit();
                () = insert_char(&mut state, c);
                refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Backspace => {
                state.clear_esc_arm();
                // If we were showing app-side preedit, drop it first (IME cancel).
                if !state.ime_preedit.is_empty() {
                    state.clear_ime_preedit();
                    continue;
                }
                () = backspace(&mut state);
                refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Left if !message.modifiers.contains(KeyModifiers::SHIFT) => {
                state.clear_esc_arm();
                state.clear_ime_preedit();
                () = move_cursor_left(&mut state);
                refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Right if !message.modifiers.contains(KeyModifiers::SHIFT) => {
                state.clear_esc_arm();
                state.clear_ime_preedit();
                () = move_cursor_right(&mut state);
                refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Home => {
                () = state.clear_esc_arm();
                state.clear_ime_preedit();
                state.cursor = 0;
                refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::End => {
                state.clear_esc_arm();
                state.clear_ime_preedit();
                state.cursor = state.prompt.len();
                refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Enter => {
                state.clear_esc_arm();
                state.clear_ime_preedit();
                // Menu open → accept only (no dispatch / no agent submit).
                if enter_should_accept_menu(&state) {
                    let _ =
                        super::slash_complete::accept_slash_candidate(&mut state, &agent_config);
                    continue;
                }
                let line = state.prompt.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                () = state.clear_prompt();
                if is_ui_quit_line(&line) {
                    exit.write_default();
                    return;
                }
                if line.starts_with('/') {
                    let session_processing = agent_busy;
                    () = handle_slash(
                        &line,
                        &mut state,
                        &mut agent_config,
                        &mut session,
                        &config,
                        &cli,
                        coding_agent.as_deref(),
                        tokio_runtime.runtime(),
                        session_processing,
                        &mut exit,
                        &prompt_channel,
                    );
                } else {
                    session.ctrl_c_armed = false;
                    () = prompt_channel.send_prompt(line);
                    () = state.set_status_hint("prompt sent");
                }
            }
            _ => {}
        }
    }
}

/// Pure Esc handling for tests + `input_system` (menu already dismissed by caller).
pub fn handle_esc(state: &mut TuiState, busy: bool, now: Instant) {
    if busy {
        // Grok: Esc does not cancel mid-turn
        return;
    }
    if state.focus != TuiFocus::Prompt {
        // Clear is prompt-pane only; scrollback Esc is no-op
        return;
    }
    if state.prompt.is_empty() {
        return;
    }
    match state.esc_armed_at {
        Some(armed) if now.duration_since(armed) <= ESC_CLEAR_WINDOW => {
            () = state.clear_prompt();
            () = state.clear_esc_arm();
            () = state.set_status_hint("prompt cleared");
        }
        _ => {
            state.esc_armed_at = Some(now);
            () = state.set_status_hint("press Esc again to clear");
        }
    }
}

fn insert_char(state: &mut TuiState, c: char) {
    let idx = state.cursor.min(state.prompt.len());
    () = state.prompt.insert(idx, c);
    state.cursor = idx + c.len_utf8();
}

/// Insert a UTF-8 string at the prompt cursor (paste / multi-char IME commit).
fn insert_str(state: &mut TuiState, s: &str) {
    if s.is_empty() {
        return;
    }
    let idx = state.cursor.min(state.prompt.len());
    () = state.prompt.insert_str(idx, s);
    state.cursor = idx + s.len();
}

fn backspace(state: &mut TuiState) {
    if state.cursor == 0 {
        return;
    }
    let idx = state.cursor;
    let prev = state.prompt[..idx]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0);
    () = state.prompt.replace_range(prev..idx, "");
    state.cursor = prev;
}

fn move_cursor_left(state: &mut TuiState) {
    if state.cursor == 0 {
        return;
    }
    let prev = state.prompt[..state.cursor]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0);
    state.cursor = prev;
}

fn move_cursor_right(state: &mut TuiState) {
    if state.cursor >= state.prompt.len() {
        return;
    }
    let next = state.prompt[state.cursor..]
        .chars()
        .next()
        .map(|c| state.cursor + c.len_utf8())
        .unwrap_or(state.prompt.len());
    state.cursor = next;
}

fn handle_slash(
    line: &str,
    state: &mut TuiState,
    agent_config: &mut AgentConfig,
    session: &mut ReplSessionState,
    config: &Config,
    cli: &Cli,
    coding_agent: Option<&CodingAgent>,
    runtime: &tokio::runtime::Runtime,
    session_processing: bool,
    exit: &mut MessageWriter<AppExit>,
    prompt_channel: &CodingAgentPromptChannel,
) {
    match dispatch_slash_command(
        line,
        agent_config,
        session,
        config,
        None,
        coding_agent,
        runtime,
        None,
        cli.bare,
        session_processing,
    ) {
        DispatchResult::Exit => {
            let _ = exit.write_default();
        }
        DispatchResult::Handled {
            output,
            detail,
            redraw_prompt: _,
            reinstall,
        } => {
            // Multi-line slash output → operator panel (not Session ECS scrollback).
            let mut lines: Vec<String> = output.into_iter().chain(detail).collect();
            if reinstall.is_some() {
                lines.push(
                    "(reinstall requested — use line REPL for full agent reinstall in foundation)"
                        .into(),
                );
            }
            let title = line
                .split_whitespace()
                .next()
                .unwrap_or("output")
                .trim_start_matches('/')
                .to_string();
            () = state.open_operator_panel(title, lines);
        }
        DispatchResult::ResendPrompt { prompt, hint } => {
            () = state.set_status_hint(hint);
            session.ctrl_c_armed = false;
            () = prompt_channel.send_prompt(prompt);
        }
        DispatchResult::AgentOp(inv) => {
            let mut lines = inv.preamble;
            () = lines.push(
                "(agent file ops simplified in TUI foundation — use line REPL for /save /load /jump)"
                    .into(),
            );
            () = state.open_operator_panel("agent", lines);
        }
        DispatchResult::AwaitClearConfirm { prompt } => {
            () = state.open_operator_panel(
                "confirm",
                [
                    prompt,
                    "clear confirm: use line REPL for multi-step confirm in foundation".into(),
                ],
            );
        }
        DispatchResult::Unknown => {
            () = state.open_operator_panel("unknown", [format!("unknown command: {line}")]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn esc_does_not_clear_when_busy() {
        let mut s = TuiState::default();
        s.prompt = "hello".into();
        s.cursor = 5;
        handle_esc(&mut s, true, Instant::now());
        assert_eq!(s.prompt, "hello");
        assert!(s.esc_armed_at.is_none());
    }

    #[test]
    fn double_esc_clears_prompt() {
        let mut s = TuiState::default();
        s.focus = TuiFocus::Prompt;
        s.prompt = "draft".into();
        s.cursor = 5;
        let t0 = Instant::now();
        handle_esc(&mut s, false, t0);
        assert_eq!(s.prompt, "draft");
        assert!(s.esc_armed_at.is_some());
        handle_esc(&mut s, false, t0 + Duration::from_millis(100));
        assert!(s.prompt.is_empty());
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn esc_on_scrollback_noops() {
        let mut s = TuiState::default();
        s.focus = TuiFocus::Scrollback;
        s.prompt = "x".into();
        handle_esc(&mut s, false, Instant::now());
        assert_eq!(s.prompt, "x");
        assert_eq!(s.focus, TuiFocus::Scrollback);
    }

    #[test]
    fn esc_empty_prompt_noop() {
        let mut s = TuiState::default();
        s.focus = TuiFocus::Prompt;
        handle_esc(&mut s, false, Instant::now());
        assert!(s.esc_armed_at.is_none());
    }

    #[test]
    fn rect_contains_hit_test() {
        use ratatui::layout::Rect;
        let r = Rect {
            x: 0,
            y: 10,
            width: 80,
            height: 3,
        };
        assert!(rect_contains(r, 0, 10));
        assert!(rect_contains(r, 79, 12));
        assert!(!rect_contains(r, 80, 10));
        assert!(!rect_contains(r, 0, 9));
    }
}
