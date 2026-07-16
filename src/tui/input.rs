//! Keyboard + mouse input via `bevy_ratatui` messages.

use {
    super::{
        commands::{accept_palette_selection, handle_theme_slash, is_theme_slash, is_ui_quit_line},
        nav::{jump_end, jump_home, jump_turn, move_selection_by, ratatui_scroll_y, scroll_page},
        prompt_history::{
            BrowseDownResult, accept as history_accept, close_restore, combine_prompt_history,
            detach as history_detach, open_browse, step_down, step_up,
            user_prompt_texts_from_blocks,
        },
        scrollback::ScrollbackView,
        slash_complete::{
            TabSlashResult, enter_should_accept_menu, handle_prompt_tab, move_menu_highlight,
            refresh_slash_completion, try_dismiss_slash_menu,
        },
        state::{
            ESC_CLEAR_WINDOW, TuiFocus, TuiState, dismiss_key_stealing_overlays,
            hit_prompt_or_status, rect_contains,
        },
    },
    crate::{
        agents::{AgentConfig, CodingAgent, CodingAgentPromptChannel},
        cli::Cli,
        config::Config,
        repl::{
            DispatchResult, ReplEcsDashboard, dispatch_slash_command,
            history::{ReplInputHistory, persist_repl_history},
            session_dashboard::SessionDashboardSnapshot,
            session_state::ReplSessionState,
        },
        session::{ContentBlock, ContentBlockEntity, SessionRuntimeStatus},
    },
    bevy::{
        app::AppExit,
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageReader, MessageWriter},
            query::With,
            system::Query,
        },
        platform::time::Instant,
    },
    bevy_ratatui::{
        crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind},
        event::{KeyMessage, MouseMessage, PasteMessage},
    },
    bevy_tokio_tasks::TokioTasksRuntime,
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
///
/// Clicks **outside** the operator panel dismiss key-stealing overlays (panel /
/// palette / cheatsheet). Otherwise focus can look like Prompt (caret drawn) while
/// printable keys are still swallowed — “click prompt but cannot type.”
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
                    // Selectable catalogs: click row to highlight + fill. Read-only: no-op.
                    if state.operator_panel.is_selectable() {
                        let rect = state.operator_panel.last_rect;
                        if rect.width > 2
                            && rect.height > 2
                            && !state.operator_panel.lines.is_empty()
                        {
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
                    }
                    continue;
                }
                // Outside panel (and any chrome click): drop overlays that steal keys.
                () = dismiss_key_stealing_overlays(&mut state);

                // Prompt + status strip both mean “I want to type”.
                if hit_prompt_or_status(
                    state.last_prompt_rect,
                    state.last_status_rect,
                    ev.column,
                    ev.row,
                ) {
                    state.focus = TuiFocus::Prompt;
                    ()  =state.clear_esc_arm();
                } else if rect_contains(state.last_scrollback_rect, ev.column, ev.row) {
                    state.focus = TuiFocus::Scrollback;
                    () = state.clear_esc_arm();
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
            // Wheel: panel → palette → scrollback.
            MouseEventKind::ScrollUp => {
                if state.operator_panel.open {
                    panel_wheel(&mut state, 1);
                } else if state.command_palette.open {
                    state.command_palette.move_highlight(1);
                } else {
                    () = move_selection_by(&mut state, line_count, 1);
                }
            }
            MouseEventKind::ScrollDown => {
                if state.operator_panel.open {
                    panel_wheel(&mut state, -1);
                } else if state.command_palette.open {
                    state.command_palette.move_highlight(-1);
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
        if state.operator_panel.open
            || state.command_palette.open
            || state.shortcuts_cheatsheet.open
        {
            continue;
        }
        if state.focus != TuiFocus::Prompt {
            continue;
        }
        state.clear_esc_arm();
        state.clear_ime_preedit();
        // Paste detaches Grok browse (keep filled text, then insert).
        if state.prompt_history.is_open() {
            () = history_detach(&mut state.prompt_history);
        }
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
    auth_ui: Res<super::auth_ui::AuthUiChannel>,
    runtime_status: Query<&SessionRuntimeStatus>,
    ecs_dashboard: ReplEcsDashboard,
    mut input_history: ResMut<ReplInputHistory>,
    content_blocks: Query<&ContentBlock, With<ContentBlockEntity>>,
) {
    let line_count = scrollback.line_count();
    let turn_starts = scrollback.turn_starts.clone();
    let agent_busy = runtime_status.iter().any(|s| s.is_processing());
    () = state.expire_esc_arm_if_stale(Instant::now());

    for message in keys.read() {
        // Ignore Null (some IME/control sequences) and non-press kinds.
        if matches!(message.code, KeyCode::Null) {
            continue;
        }
        if message.kind != KeyEventKind::Press && message.kind != KeyEventKind::Repeat {
            continue;
        }

        // ── Ctrl+P palette toggle · Ctrl+X / Ctrl+. cheatsheet ───────────
        if message.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(message.code, KeyCode::Char('p') | KeyCode::Char('P'))
        {
            () = state.clear_esc_arm();
            () = state.clear_ime_preedit();
            // Operator panel stays above palette in Esc order; still allow open
            // so discover works, but panel remains on top until Esc.
            () = state.toggle_command_palette();
            continue;
        }
        if message.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(
                message.code,
                KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Char('.')
            )
        {
            () = state.clear_esc_arm();
            () = state.clear_ime_preedit();
            if state.shortcuts_cheatsheet.open {
                () =  state.close_shortcuts_cheatsheet();
            } else {
                () = state.open_shortcuts_cheatsheet();
            }
            continue;
        }

        // ── Ctrl+C / Ctrl+D (foundation) ──────────────────────────────────
        if message.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(message.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            state.clear_esc_arm();
            if session.ctrl_c_armed {
                if let Some(handle) = session.active_shell.as_mut() {
                    () =  crate::repl::shell_run::request_shell_interrupt(handle);
                    () = crate::repl::shell_run::request_shell_interrupt(handle);
                }
                session.active_shell = None;
                session.ctrl_c_armed = false;
                exit.write_default();
                return;
            }

            if let Some(handle) = session.active_shell.as_mut() {
                () = crate::repl::shell_run::request_shell_interrupt(handle);
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

        // ── Esc: IME → panel → theme picker → palette → cheatsheet → history → slash → clear ─
        if message.code == KeyCode::Esc {
            if !state.ime_preedit.is_empty() {
                // Cancel composition first (do not clear the committed prompt).
                () = state.clear_ime_preedit();
                continue;
            }
            if state.operator_panel.open {
                () = state.close_operator_panel();
                continue;
            }
            if state.theme_picker.open {
                () = state.cancel_theme_picker();
                continue;
            }
            if state.command_palette.open {
                () = state.close_command_palette();
                continue;
            }
            if state.shortcuts_cheatsheet.open {
                () = state.close_shortcuts_cheatsheet();
                continue;
            }
            if state.prompt_history.is_open() {
                let restore = close_restore(&mut state.prompt_history);
                () = state.set_prompt_fill(restore);
                () = state.clear_esc_arm();
                continue;
            }
            if try_dismiss_slash_menu(&mut state) {
                () = refresh_slash_completion(&mut state, &agent_config, false);
                // Count menu dismiss as the first Esc of double-clear so
                // `/` + 2×Esc closes the menu and clears the draft (not 3×Esc).
                let now = Instant::now();
                if !state.prompt.is_empty() && state.focus == TuiFocus::Prompt {
                    () = state.arm_esc_clear(now);
                }
                continue;
            }
            if let Some(cleared) = handle_esc(
                &mut state,
                agent_busy || session.active_shell.is_some(),
                Instant::now(),
            ) {
                // Grok: 2×Esc clear records draft into prompt history bag.
                () = input_history.push_submitted(&cleared);
                () = persist_repl_history(
                    input_history.as_mut(),
                    &crate::config_paths::repl_history_path(),
                );
            }
            continue;
        }

        // ── Theme picker (live preview; Enter apply+persist; Esc cancel) ─
        if state.theme_picker.open {
            match message.code {
                KeyCode::Up => state.theme_picker.move_highlight(-1),
                KeyCode::Down => state.theme_picker.move_highlight(1),
                KeyCode::Home => state.theme_picker.highlight = 0,
                KeyCode::End => {
                    state.theme_picker.highlight =
                        super::theme::CATALOG.len().saturating_sub(1);
                }
                KeyCode::Enter => {
                    let id = state.apply_theme_picker_selection();
                    () = config.set_theme(id.as_str());
                }
                _ => {}
            }
            continue;
        }

        // ── Command palette (search field + list; Enter fills, does not dispatch)
        if state.command_palette.open {
            match message.code {
                KeyCode::Up => state.command_palette.move_highlight(-1),
                KeyCode::Down => state.command_palette.move_highlight(1),
                KeyCode::PageUp => state.command_palette.move_highlight(-10),
                KeyCode::PageDown => state.command_palette.move_highlight(10),
                // Home/End move list selection (search cursor is always at end of query).
                KeyCode::Home => state.command_palette.highlight = 0,
                KeyCode::End => {
                    let n = state.command_palette.rows.len();
                    state.command_palette.highlight = n.saturating_sub(1);
                }
                KeyCode::Enter => {
                    if accept_palette_selection(
                        &mut state,
                        config.as_ref(),
                        tokio_runtime.as_ref(),
                        auth_ui.as_ref(),
                    ) {
                        () = refresh_slash_completion(&mut state, &agent_config, false);
                    }
                }
                KeyCode::Backspace => {
                    () = state.command_palette.filter_backspace();
                }
                KeyCode::Delete => {
                    // Same as backspace for end-of-query search field.
                    () = state.command_palette.filter_backspace();
                }
                KeyCode::Char(c)
                    if !message.modifiers.contains(KeyModifiers::CONTROL)
                        && !message.modifiers.contains(KeyModifiers::ALT)
                        && !c.is_control() =>
                {
                    // All printable typing goes into the in-palette search field.
                    () = state.command_palette.insert_filter_char(c);
                }
                _ => {}
            }
            continue;
        }

        // ── Shortcuts cheatsheet: Esc already handled; swallow other keys ─
        if state.shortcuts_cheatsheet.open {
            // Allow opening palette on top via Ctrl+P already handled above.
            continue;
        }

        // ── Operator panel (separate window; not scrollback) ─────────────
        if state.operator_panel.open {
            if !state.operator_panel.is_selectable() {
                // Read-only dump (/tokens, …): any key closes so typing continues.
                () = dismiss_readonly_panel(
                    &mut state,
                    &agent_config,
                    message.code,
                    message.modifiers,
                );
                continue;
            }
            match message.code {
                KeyCode::Up => panel_nav(&mut state, -1),
                KeyCode::Down => panel_nav(&mut state, 1),
                KeyCode::PageUp => panel_nav(&mut state, -10),
                KeyCode::PageDown => panel_nav(&mut state, 10),
                KeyCode::Home => {
                    state.operator_panel.selected = 0;
                    () = state.sync_prompt_from_operator_selection();
                }
                KeyCode::End => {
                    let n = state.operator_panel.lines.len();
                    state.operator_panel.selected = n.saturating_sub(1);
                    () = state.sync_prompt_from_operator_selection();
                }
                KeyCode::Enter => {
                    // Fill prompt from selection; close panel; stay ready to run.
                    () = state.sync_prompt_from_operator_selection();
                    () = state.close_operator_panel();
                    state.focus = TuiFocus::Prompt;
                    () = refresh_slash_completion(&mut state, &agent_config, false);
                }
                KeyCode::Tab => {
                    state.focus = TuiFocus::Prompt;
                    () = state.clear_esc_arm();
                    () = refresh_slash_completion(&mut state, &agent_config, false);
                }
                _ => {}
            }
            continue;
        }

        // ── Prompt history browse (Grok empty-↑ modal) ────────────────────
        if state.prompt_history.is_open() {
            match message.code {
                KeyCode::Up => {
                    if let Some(fill) = step_up(&mut state.prompt_history) {
                        () = state.set_prompt_fill(fill);
                    }
                }
                KeyCode::Down => match step_down(&mut state.prompt_history) {
                    Some(BrowseDownResult::StillOpen { fill }) => {
                        () = state.set_prompt_fill(fill);
                    }
                    Some(BrowseDownResult::Closed { restore }) => {
                        () = state.set_prompt_fill(restore);
                    }
                    None => {}
                },
                KeyCode::Enter | KeyCode::Tab => {
                    let text = history_accept(&mut state.prompt_history);
                    () = state.set_prompt_fill(text);
                    // Accept only — do not send / dispatch on this key.
                }
                KeyCode::Char(c)
                    if !message.modifiers.contains(KeyModifiers::CONTROL)
                        && !message.modifiers.contains(KeyModifiers::ALT)
                        && !c.is_control() =>
                {
                    // Detach: keep filled text, apply key as normal edit.
                    () = history_detach(&mut state.prompt_history);
                    () = state.clear_esc_arm();
                    () = state.clear_ime_preedit();
                    if c == '?' && state.prompt.is_empty() {
                        state.open_command_palette();
                    } else {
                        () = insert_char(&mut state, c);
                        () = refresh_slash_completion(&mut state, &agent_config, false);
                    }
                }
                KeyCode::Backspace => {
                    () = history_detach(&mut state.prompt_history);
                    () = state.clear_esc_arm();
                    if !state.ime_preedit.is_empty() {
                        state.clear_ime_preedit();
                    } else {
                        () = backspace(&mut state);
                        () = refresh_slash_completion(&mut state, &agent_config, false);
                    }
                }
                KeyCode::Left if !message.modifiers.contains(KeyModifiers::SHIFT) => {
                    () = history_detach(&mut state.prompt_history);
                    () = state.clear_esc_arm();
                    () = state.clear_ime_preedit();
                    () = move_cursor_left(&mut state);
                }
                KeyCode::Right if !message.modifiers.contains(KeyModifiers::SHIFT) => {
                    () = history_detach(&mut state.prompt_history);
                    () = state.clear_esc_arm();
                    () = state.clear_ime_preedit();
                    () = move_cursor_right(&mut state);
                }
                _ => {
                    // Other keys: detach and fall through? Prefer consume.
                }
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
                    () = state.clear_esc_arm();
                    () = refresh_slash_completion(&mut state, &agent_config, false);
                }
                KeyCode::Char(' ')
                    if !message
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    state.focus = TuiFocus::Prompt;
                    () = state.clear_esc_arm();
                    () = refresh_slash_completion(&mut state, &agent_config, false);
                }
                KeyCode::Up => move_selection_by(&mut state, line_count, -1),
                KeyCode::Down => move_selection_by(&mut state, line_count, 1),
                KeyCode::Left if message.modifiers.contains(KeyModifiers::SHIFT) => {
                    () =  jump_turn(&mut state, &turn_starts, line_count, false);
                }
                KeyCode::Right if message.modifiers.contains(KeyModifiers::SHIFT) => {
                    () =  jump_turn(&mut state, &turn_starts, line_count, true);
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
                    if c == '?' && state.prompt.is_empty() {
                        () = state.open_command_palette();
                    } else {
                        () =  insert_char(&mut state, c);
                        () = refresh_slash_completion(&mut state, &agent_config, false);
                    }
                }
                _ => {}
            }
            continue;
        }

        // ── Prompt-focused: menu arrows before edit keys ──────────────────
        if state.slash_menu.open {
            match message.code {
                KeyCode::Up => {
                    () = move_menu_highlight(&mut state, -1);
                    continue;
                }
                KeyCode::Down => {
                    () = move_menu_highlight(&mut state, 1);
                    continue;
                }
                _ => {}
            }
        }

        // ── Empty ↑: open Grok prompt-history browse (↓ never opens) ─────
        if state.focus == TuiFocus::Prompt
            && state.prompt.is_empty()
            && !state.slash_menu.open
            && message.code == KeyCode::Up
            && !message.modifiers.contains(KeyModifiers::CONTROL)
            && !message.modifiers.contains(KeyModifiers::ALT)
        {
            let ecs_user = user_prompt_texts_from_blocks(content_blocks.iter());
            let combined = combine_prompt_history(&ecs_user, input_history.entries());
            if let Some(browse) = open_browse(combined, String::new()) {
                let fill = browse.selected_text().unwrap_or("").to_owned();
                state.prompt_history = browse;
                () = state.set_prompt_fill(fill);
            }
            // Consumed even when empty (no cursor motion fallback).
            continue;
        }
        // Explicit: ↓ on empty closed prompt does not open history.
        if state.focus == TuiFocus::Prompt
            && state.prompt.is_empty()
            && !state.slash_menu.open
            && message.code == KeyCode::Down
            && !message.modifiers.contains(KeyModifiers::CONTROL)
            && !message.modifiers.contains(KeyModifiers::ALT)
        {
            continue;
        }

        // ── Prompt-focused edit ───────────────────────────────────────────
        match message.code {
            KeyCode::Tab => {
                () = state.clear_esc_arm();
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
                () = state.clear_esc_arm();
                () = state.clear_ime_preedit();
                // `?` opens palette when draft is empty (Grok agent-screen binding).
                if c == '?' && state.prompt.is_empty() {
                    () = state.open_command_palette();
                    continue;
                }
                () = insert_char(&mut state, c);
                refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Backspace => {
                () = state.clear_esc_arm();
                // If we were showing app-side preedit, drop it first (IME cancel).
                if !state.ime_preedit.is_empty() {
                    () =   state.clear_ime_preedit();
                    continue;
                }
                () = backspace(&mut state);
                () = refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Left if !message.modifiers.contains(KeyModifiers::SHIFT) => {
                () = state.clear_esc_arm();
                () = state.clear_ime_preedit();
                () = move_cursor_left(&mut state);
                () = refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Right if !message.modifiers.contains(KeyModifiers::SHIFT) => {
                () =    state.clear_esc_arm();
                () =    state.clear_ime_preedit();
                () = move_cursor_right(&mut state);
                () =  refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Home => {
                () = state.clear_esc_arm();
                () = state.clear_ime_preedit();
                state.cursor = 0;
                () = refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::End => {
                () = state.clear_esc_arm();
                ()  = state.clear_ime_preedit();
                state.cursor = state.prompt.len();
                ()  =  refresh_slash_completion(&mut state, &agent_config, false);
            }
            KeyCode::Enter => {
                ()  = state.clear_esc_arm();
                ()  = state.clear_ime_preedit();
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
                // Shared bag with line REPL (Grok / readline history).
                () = input_history.push_submitted(&line);
                () = persist_repl_history(
                    input_history.as_mut(),
                    &crate::config_paths::repl_history_path(),
                );
                () = state.clear_prompt();
                if is_ui_quit_line(&line) {
                    exit.write_default();
                    return;
                }
                if is_theme_slash(&line) {
                    let _ = handle_theme_slash(&line, &mut state, config.as_ref());
                    continue;
                }
                if line.starts_with('/') {
                    let runtime = tokio_runtime.runtime();
                    let session_id = coding_agent
                        .as_ref()
                        .map(|a| a.session_id())
                        .unwrap_or_default();
                    // Match line REPL: info slash needs Session ECS snapshot (not None).
                    let dashboard = if slash_needs_dashboard(&line) {
                        Some(ecs_dashboard.info_snapshot(
                            session_id,
                            &agent_config.model,
                            coding_agent.as_deref(),
                            runtime,
                        ))
                    } else {
                        None
                    };
                    let session_processing = coding_agent
                        .as_ref()
                        .map(|a| ecs_dashboard.is_processing(a.session_id()))
                        .unwrap_or(agent_busy);
                    () = handle_slash(
                        &line,
                        &mut state,
                        &mut agent_config,
                        &mut session,
                        &config,
                        &cli,
                        coding_agent.as_deref(),
                        runtime,
                        dashboard,
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
///
/// Returns `Some(cleared_text)` when double-Esc successfully clears a non-empty draft
/// (caller records into `ReplInputHistory`).
pub fn handle_esc(state: &mut TuiState, busy: bool, now: Instant) -> Option<String> {
    if busy {
        // Grok: Esc does not cancel mid-turn
        return None;
    }
    if state.focus != TuiFocus::Prompt {
        // Clear is prompt-pane only; scrollback Esc is no-op
        return None;
    }
    if state.prompt.is_empty() {
        return None;
    }
    match state.esc_armed_at {
        Some(armed) if now.duration_since(armed) <= ESC_CLEAR_WINDOW => {
            let cleared = state.prompt.clone();
            () = state.clear_prompt();
            () = state.clear_esc_arm();
            // Back to idle key chrome (not a sticky toast that hides bindings).
            state.status_hint = None;
            Some(cleared)
        }
        _ => {
            () = state.arm_esc_clear(now);
            None
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

/// `/status` `/tokens` `/cost` `/context` need a prebuilt Session ECS dashboard.
fn slash_needs_dashboard(line: &str) -> bool {
    let cmd = line.split_whitespace().next().unwrap_or(line).trim();
    matches!(cmd, "/status" | "/tokens" | "/cost" | "/context")
}

/// Catalog panels (`/help`) keep select+fill; dumps are read-only.
fn slash_panel_is_selectable(line: &str) -> bool {
    let cmd = line.split_whitespace().next().unwrap_or(line).trim();
    matches!(cmd, "/help")
}

fn panel_viewport_h(state: &TuiState) -> usize {
    (state.operator_panel.last_rect.height.saturating_sub(2) as usize).max(1)
}

fn panel_nav(state: &mut TuiState, delta: isize) {
    if state.operator_panel.is_selectable() {
        ()  = state.operator_panel.move_selection(delta);
        ()  =  state.sync_prompt_from_operator_selection();
    } else {
        let vh = panel_viewport_h(state);
        ()  = state.operator_panel.scroll_by(delta, vh);
    }
}

fn panel_wheel(state: &mut TuiState, delta: isize) {
    // Wheel still scrolls long read-only dumps; keys dismiss (see dismiss_readonly_panel).
    panel_nav(state, delta);
}

/// Close a read-only panel; printable keys land in the prompt so typing continues.
fn dismiss_readonly_panel(
    state: &mut TuiState,
    agent_config: &crate::agents::AgentConfig,
    code: KeyCode,
    modifiers: KeyModifiers,
) {
    ()  = state.close_operator_panel();
    state.focus = TuiFocus::Prompt;
    ()  = state.clear_esc_arm();
    ()  = state.clear_ime_preedit();
    if let KeyCode::Char(c) = code
        && !modifiers.contains(KeyModifiers::CONTROL)
        && !modifiers.contains(KeyModifiers::ALT)
        && !c.is_control()
    {
        // Insert into prompt so the same keystroke starts the next input.
        // (`?` on empty draft would open palette if handled later; insert here instead.)
        () = insert_char(state, c);
        refresh_slash_completion(state, agent_config, false);
    }
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
    dashboard: Option<SessionDashboardSnapshot>,
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
        dashboard,
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
            if slash_panel_is_selectable(line) {
                () = state.open_operator_panel(title, lines);
            } else {
                // /tokens /status /cost /… — scrollable dump, not a pick-list.
                () = state.open_operator_panel_readonly(title, lines);
            }
        }
        DispatchResult::ResendPrompt { prompt, hint } => {
            () = state.set_status_hint(hint);
            session.ctrl_c_armed = false;
            () = prompt_channel.send_prompt(prompt);
        }
        DispatchResult::AgentOp(inv) => {
            let mut lines = inv.preamble;
            () = lines.push(
                String::from("(agent file ops simplified in TUI foundation — use line REPL for /save /load /jump)"),
            );
            () = state.open_operator_panel_readonly("agent", lines);
        }
        DispatchResult::AwaitClearConfirm { prompt } => {
            () = state.open_operator_panel_readonly(
                "confirm",
                [
                    prompt,
                    String::from(
                        "clear confirm: use line REPL for multi-step confirm in foundation",
                    ),
                ],
            );
        }
        DispatchResult::Unknown => {
            () =
                state.open_operator_panel_readonly("unknown", [format!("unknown command: {line}")]);
        }
    }
}
