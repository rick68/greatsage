use {
    super::RenderNeeded,
    crate::agents::{CodingAgentPromptChannel, CodingAgentTotalTokenUsage},
    bevy::{
        app::{App, AppExit, PreUpdate, Update},
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            message::{MessageReader, MessageWriter},
            schedule::IntoScheduleConfigs,
            system::Local,
        },
        state::{
            app::AppExtStates,
            condition::in_state,
            state::{NextState, State, States},
        },
        time::{Time, Timer, TimerMode},
    },
    bevy_ratatui::{
        RatatuiContext, crossterm,
        event::{KeyMessage, MouseMessage},
    },
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        style::{Style, Stylize},
        text::{Line, Span},
        widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    },
    std::{
        iter::{DoubleEndedIterator, ExactSizeIterator, Iterator},
        time::Duration,
    },
    strum::{EnumCount, FromRepr},
    unicode_width::{UnicodeWidthChar, UnicodeWidthStr},
};

const CURSOR_BLINK_INTERVAL_MS: u64 = 530;
const PROMPT_PREFIX: &str = "🤖 > ";

#[derive(Clone, Copy, Debug, Default, EnumCount, Eq, FromRepr, Hash, PartialEq, States)]
#[repr(u8)]
pub enum TuiMainFocus {
    #[default]
    InputArea,
    OutputArea,
}

impl Iterator for TuiMainFocus {
    type Item = TuiMainFocus;

    fn next(&mut self) -> Option<Self::Item> {
        let next = ((*self as usize + 1) % Self::COUNT) as u8;
        TuiMainFocus::from_repr(next).inspect(|item| *self = *item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (Self::COUNT, Some(Self::COUNT))
    }
}

impl ExactSizeIterator for TuiMainFocus {}

impl DoubleEndedIterator for TuiMainFocus {
    fn next_back(&mut self) -> Option<Self::Item> {
        let next = if (*self as u8) == 0 {
            (Self::COUNT - 1) as u8
        } else {
            (*self as u8) - 1
        };
        TuiMainFocus::from_repr(next).inspect(|item| *self = *item)
    }
}

#[derive(Default)]
pub struct TuiMain<'a> {
    input: String,
    byte_index: usize, // byte offset of cursor in input
    history: Vec<String>,
    history_pos: Option<usize>, // None = editing new input, Some(i) = viewing history[i]
    history_draft: String,      // saved draft when navigating history
    pub output: Vec<Line<'a>>,
    output_area: Rect,
    show_cursor: bool,
    focused: TuiMainFocus,
    vertical_scroll: usize,
    vertical_scroll_state: ScrollbarState,
}

impl<'a> TuiMain<'a> {
    #[cfg(test)]
    pub(crate) fn set_input_public(&mut self, s: String) {
        self.set_input(s);
    }
    #[cfg(test)]
    pub(crate) fn push_history_public(&mut self, s: &str) {
        self.push_history(s);
    }
    #[cfg(test)]
    pub(crate) fn clear_input_public(&mut self) {
        self.clear_input();
    }
    #[cfg(test)]
    pub(crate) fn input_is_empty(&self) -> bool {
        self.input.is_empty()
    }
    #[cfg(test)]
    pub(crate) fn last_history(&self) -> Option<&str> {
        self.history.last().map(|s| s.as_str())
    }

    fn display_index(&self) -> usize {
        self.input[..self.byte_index].width()
    }

    fn input_display_lines(&self, inner_width: usize) -> Vec<String> {
        let display_text = format!("{PROMPT_PREFIX}{}", self.input);
        if inner_width == 0 {
            return vec![display_text];
        }
        let mut lines: Vec<String> = Vec::new();
        let mut current_line = String::new();
        let mut current_width = 0usize;
        for ch in display_text.chars() {
            let ch_w = ch.width().unwrap_or(1);
            if current_width + ch_w > inner_width && !current_line.is_empty() {
                lines.push(std::mem::take(&mut current_line));
                current_width = 0;
            }
            current_line.push(ch);
            current_width += ch_w;
        }
        lines.push(current_line);
        lines
    }

    fn hard_wrap_output_lines<'b>(lines: &'b [Line<'b>], inner_width: usize) -> Vec<Line<'static>> {
        if inner_width == 0 {
            return lines.iter().map(|l| Line::from(l.to_string())).collect();
        }
        let mut result: Vec<Line<'static>> = Vec::new();
        for line in lines {
            let mut current_spans: Vec<Span<'static>> = Vec::new();
            let mut current_width = 0usize;
            for span in &line.spans {
                let style = span.style;
                let mut buf = String::new();
                for ch in span.content.chars() {
                    let ch_w = ch.width().unwrap_or(1);
                    if current_width + ch_w > inner_width && current_width > 0 {
                        if !buf.is_empty() {
                            current_spans.push(Span::styled(std::mem::take(&mut buf), style));
                        }
                        result.push(Line::from(std::mem::take(&mut current_spans)));
                        current_width = 0;
                    }
                    buf.push(ch);
                    current_width += ch_w;
                }
                if !buf.is_empty() {
                    current_spans.push(Span::styled(buf, style));
                }
            }
            result.push(Line::from(current_spans));
        }
        result
    }

    fn insert_char(&mut self, c: char) {
        self.input.insert(self.byte_index, c);
        self.byte_index += c.len_utf8();
    }

    fn delete_before(&mut self) {
        if self.byte_index == 0 {
            return;
        }
        let c = self.input[..self.byte_index].chars().next_back().unwrap();
        self.byte_index -= c.len_utf8();
        self.input.remove(self.byte_index);
    }

    fn delete_after(&mut self) {
        if self.byte_index < self.input.len() {
            self.input.remove(self.byte_index);
        }
    }

    fn cursor_left(&mut self) {
        if let Some(c) = self.input[..self.byte_index].chars().next_back() {
            self.byte_index -= c.len_utf8();
        }
    }

    fn cursor_right(&mut self) {
        if let Some(c) = self.input[self.byte_index..].chars().next() {
            self.byte_index += c.len_utf8();
        }
    }

    fn cursor_to_start(&mut self) {
        self.byte_index = 0;
    }

    fn cursor_to_end(&mut self) {
        self.byte_index = self.input.len();
    }

    fn set_input(&mut self, s: String) {
        self.input = s;
        self.cursor_to_end();
    }

    fn clear_input(&mut self) {
        self.input.clear();
        self.byte_index = 0;
    }

    /// Clears the REPL output buffer and resets scrolling.
    fn clear_output(&mut self) {
        self.output.clear();
        self.vertical_scroll = 0;
        self.vertical_scroll_state = ScrollbarState::default();
    }

    fn push_history(&mut self, s: &str) {
        if !s.is_empty() && self.history.last().map(|l| l != s).unwrap_or(true) {
            self.history.push(s.to_owned());
        }
        self.history_pos = None;
        self.history_draft.clear();
    }

    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let new_pos = match self.history_pos {
            None => {
                self.history_draft = self.input.clone();
                self.history.len() - 1
            }
            Some(0) => return,
            Some(i) => i - 1,
        };
        self.history_pos = Some(new_pos);
        let entry = self.history[new_pos].clone();
        self.set_input(entry);
    }

    fn history_next(&mut self) {
        match self.history_pos {
            None => (),
            Some(i) if i + 1 >= self.history.len() => {
                self.history_pos = None;
                let draft = self.history_draft.clone();
                self.set_input(draft);
            }
            Some(i) => {
                self.history_pos = Some(i + 1);
                let entry = self.history[i + 1].clone();
                self.set_input(entry);
            }
        }
    }

    fn output_area_height(&self) -> usize {
        const BORDER: u16 = 2;
        self.output_area.height.saturating_sub(BORDER) as usize
    }

    fn total_visual_rows(&self) -> usize {
        let inner_width = self.output_area.width.saturating_sub(2) as usize;
        if inner_width == 0 {
            return self.output.len();
        }
        let mut count = 0usize;
        for line in &self.output {
            let mut current_width = 0usize;
            let mut has_content = false;
            for span in &line.spans {
                for ch in span.content.chars() {
                    let ch_w = ch.width().unwrap_or(1);
                    if current_width + ch_w > inner_width && has_content {
                        count += 1;
                        current_width = 0;
                    }
                    current_width += ch_w;
                    has_content = true;
                }
            }
            count += 1;
        }
        count
    }

    fn max_scroll(&self) -> usize {
        self.total_visual_rows()
            .saturating_sub(self.output_area_height())
    }

    fn draw(&mut self, frame: &mut Frame, token_usage: Option<&CodingAgentTotalTokenUsage>) {
        let area = frame.area();
        let inner_width = area.width.saturating_sub(2) as usize;
        let input_lines = self.input_display_lines(inner_width);
        let input_height = (input_lines.len() as u16 + 2).max(3);

        let vertical = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(input_height),
        ]);
        let [output_area, status_area, input_area] = vertical.areas(area);
        self.output_area = output_area;

        let output_inner_width = output_area.width.saturating_sub(2) as usize;
        let wrapped = Self::hard_wrap_output_lines(&self.output, output_inner_width);
        let total_rows = wrapped.len();
        let output = Paragraph::new(wrapped)
            .style(Style::default())
            .block(Block::bordered().title("Output"))
            .scroll((self.vertical_scroll as u16, 0));
        let scroll_positions = total_rows.saturating_sub(self.output_area_height()) + 1;
        self.vertical_scroll_state = self
            .vertical_scroll_state
            .content_length(scroll_positions)
            .viewport_content_length(self.output_area_height())
            .position(self.vertical_scroll);

        () = frame.render_widget(output, output_area);
        () = frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            output_area,
            &mut self.vertical_scroll_state,
        );

        // Draw status bar with token usage
        let status_text = if let Some(usage) = token_usage {
            let CodingAgentTotalTokenUsage(usage) = usage;
            format!(
                " 🎯 Input: {} | Output: {} | Cache Read: {} | Cache Write: {}",
                usage.input, usage.output, usage.cache_read, usage.cache_write
            )
        } else {
            " 🎯 Token usage: Waiting for first response...".to_string()
        };
        let status = Paragraph::new(status_text)
            .style(Style::default().fg(ratatui::style::Color::Rgb(100, 150, 200)))
            .block(Block::bordered().title("Token Usage"));
        () = frame.render_widget(status, status_area);

        let input_text: Vec<ratatui::text::Line<'_>> = input_lines
            .iter()
            .map(|l| ratatui::text::Line::from(l.as_str()))
            .collect();
        let input = Paragraph::new(input_text)
            .style(Style::default())
            .block(Block::bordered().title("Input"));
        () = frame.render_widget::<Paragraph<'_>>(input, input_area);

        if self.show_cursor && self.focused == TuiMainFocus::InputArea {
            let cursor_total = PROMPT_PREFIX.width() + self.display_index();
            // Walk actual line widths instead of dividing by inner_width.
            // Wide chars can leave a gap at line end, making simple division wrong.
            let (cursor_row, cursor_col) = {
                let mut accumulated = 0usize;
                let mut result = (0usize, cursor_total);
                for (row, line) in input_lines.iter().enumerate() {
                    let line_w = line.width();
                    if cursor_total <= accumulated + line_w {
                        result = (row, cursor_total - accumulated);
                        break;
                    }
                    if row + 1 < input_lines.len() {
                        accumulated += line_w;
                    } else {
                        result = (row, cursor_total - accumulated);
                    }
                }
                result
            };
            () = frame.set_cursor_position((
                input_area.left() + cursor_col as u16 + 1,
                input_area.top() + cursor_row as u16 + 1,
            ));
        }
    }

    fn scroll_up(&mut self) {
        self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        if self.vertical_scroll < self.max_scroll() {
            self.vertical_scroll = self.vertical_scroll.saturating_add(1);
        }
    }

    fn scroll_page_up(&mut self) {
        self.vertical_scroll = self
            .vertical_scroll
            .saturating_sub(self.output_area_height());
    }

    fn scroll_page_down(&mut self) {
        self.vertical_scroll =
            (self.vertical_scroll + self.output_area_height()).min(self.max_scroll());
    }

    fn scroll_to_top(&mut self) {
        self.vertical_scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.vertical_scroll = self.max_scroll();
    }
}

fn handle_global_input(
    mut messages: MessageReader<KeyMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut next_tui_main_focus: ResMut<NextState<TuiMainFocus>>,
    mut exit: MessageWriter<AppExit>,
    focus: Res<State<TuiMainFocus>>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    for message in messages.read() {
        let KeyEvent {
            code,
            kind,
            modifiers,
            ..
        } = &**message;

        let in_input = focus.get() == &TuiMainFocus::InputArea;

        match code {
            KeyCode::Tab => {
                let TuiMain { focused, .. } = tui_main.as_mut();
                let next = focused.next().unwrap();
                () = next_tui_main_focus.set(next);
                **dirty = true;
            }
            KeyCode::Char('c')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                _ = exit.write_default();
            }
            // Up/Down/Home/End are yielded to InputArea for cursor/history when focused there.
            KeyCode::Up
                if !in_input && (kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat) =>
            {
                () = tui_main.scroll_up();
                **dirty = true;
            }
            KeyCode::Down
                if !in_input && (kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat) =>
            {
                () = tui_main.scroll_down();
                **dirty = true;
            }
            KeyCode::PageUp => {
                () = tui_main.scroll_page_up();
                **dirty = true;
            }
            KeyCode::PageDown => {
                () = tui_main.scroll_page_down();
                **dirty = true;
            }
            KeyCode::Home if !in_input => {
                () = tui_main.scroll_to_top();
                **dirty = true;
            }
            KeyCode::End if !in_input => {
                () = tui_main.scroll_to_bottom();
                **dirty = true;
            }
            _ => (),
        }
    }
}

fn parse_commit_message(arg: &str) -> String {
    let arg = arg.trim();
    if let Some(after_m) = arg.strip_prefix("-m") {
        after_m
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string()
    } else if let Some(idx) = arg.find("-m ") {
        arg[idx + 3..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string()
    } else {
        String::new()
    }
}

fn help_lines() -> Vec<Line<'static>> {
    vec![
        Line::raw(""),
        Line::from("Commands (in REPL):".bold()),
        Line::raw(""),
        Line::from("  Session:".cyan().bold()),
        Line::raw("    /help              Show this help"),
        Line::raw("    /clear             Clear output"),
        Line::raw("    /quit, /exit       Exit greatsage"),
        Line::raw(""),
        Line::from("  Git:".cyan().bold()),
        Line::raw("    /git stage         Stage all changes"),
        Line::raw("    /git commit -m …   Commit staged changes"),
        Line::raw("    /git revert        Revert last commit"),
        Line::raw(""),
    ]
}

fn handle_git_subcmd(cmd: &str) -> Vec<Line<'static>> {
    let rest = cmd.trim_start_matches("/git").trim();
    let (subcmd, arg) = rest
        .split_once(' ')
        .map(|(s, a)| (s.trim(), a.trim()))
        .unwrap_or((rest, ""));

    match subcmd {
        "stage" => match crate::git::stage_all() {
            Ok(_) => vec![Line::from("✅ Staged all changes").green()],
            Err(e) => vec![Line::from(format!("❌ /git stage failed: {e}")).red()],
        },
        "commit" => {
            let msg = parse_commit_message(arg);
            if msg.is_empty() {
                vec![Line::from("❌ /git commit missing -m message").red()]
            } else {
                match crate::git::commit(&msg) {
                    Ok(_) => vec![Line::from(format!("✅ Commit: {msg}")).green()],
                    Err(e) => vec![Line::from(format!("❌ /git commit failed: {e}")).red()],
                }
            }
        }
        "revert" => match crate::git::revert_last() {
            Ok(_) => vec![Line::from("✅ Reverted last commit").green()],
            Err(e) => vec![Line::from(format!("❌ /git revert failed: {e}")).red()],
        },
        "" => help_lines(),
        _ => vec![Line::from(format!("❌ Unknown /git subcommand: {subcmd}")).red()],
    }
}

pub(crate) fn handle_slash_command(cmd: &str) -> Vec<Line<'static>> {
    let base = cmd.split_whitespace().next().unwrap_or(cmd);
    match base {
        "/help" => help_lines(),
        "/git" => handle_git_subcmd(cmd),
        _ => vec![Line::from(format!("❌ Unknown command: {cmd}")).red()],
    }
}

fn handle_input_area_input(
    mut messages: MessageReader<KeyMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut exit: MessageWriter<AppExit>,
    channel: Res<CodingAgentPromptChannel>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    for message in messages.read() {
        let KeyEvent {
            code,
            kind,
            modifiers,
            ..
        } = &**message;

        let is_active = kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat;

        match code {
            KeyCode::Char(c) if is_active && !modifiers.contains(KeyModifiers::CONTROL) => {
                tui_main.insert_char(*c);
                **dirty = true;
            }
            KeyCode::Char('a')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                tui_main.cursor_to_start();
                **dirty = true;
            }
            KeyCode::Char('e')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                tui_main.cursor_to_end();
                **dirty = true;
            }
            KeyCode::Backspace if is_active => {
                tui_main.delete_before();
                **dirty = true;
            }
            KeyCode::Delete if is_active => {
                tui_main.delete_after();
                **dirty = true;
            }
            KeyCode::Left if is_active => {
                tui_main.cursor_left();
                **dirty = true;
            }
            KeyCode::Right if is_active => {
                tui_main.cursor_right();
                **dirty = true;
            }
            KeyCode::Home if matches!(kind, KeyEventKind::Press) => {
                tui_main.cursor_to_start();
                **dirty = true;
            }
            KeyCode::End if matches!(kind, KeyEventKind::Press) => {
                tui_main.cursor_to_end();
                **dirty = true;
            }
            KeyCode::Up if matches!(kind, KeyEventKind::Press) => {
                tui_main.history_prev();
                **dirty = true;
            }
            KeyCode::Down if matches!(kind, KeyEventKind::Press) => {
                tui_main.history_next();
                **dirty = true;
            }
            KeyCode::Enter if matches!(kind, KeyEventKind::Press) => {
                if !tui_main.input.is_empty() {
                    let input = tui_main.input.clone();
                    let trimmed = input.trim();

                    match trimmed {
                        "/exit" | "/quit" => {
                            () = tui_main.clear_input();
                            _ = exit.write_default();
                        }
                        "/clear" => {
                            () = tui_main.clear_output();
                            () = tui_main.clear_input();
                        }
                        cmd if cmd.starts_with('/') => {
                            let lines = handle_slash_command(cmd);
                            () = tui_main.push_history(&input);
                            for line in lines {
                                () = tui_main.output.push(line);
                            }
                            () = tui_main.clear_input();
                            () = tui_main.scroll_to_bottom();
                        }
                        _ => {
                            () = tui_main.push_history(&input);
                            () = tui_main.output.push(Line::raw(input.clone()));
                            () = tui_main.clear_input();
                            () = tui_main.scroll_to_bottom();
                            match channel.sender.send(input) {
                                Ok(_) => {}
                                Err(e) => {
                                    eprintln!("Failed to send REPL input to agent: {e}");
                                    let err_line =
                                        Line::from(format!("❌ Failed to send input: {e}")).red();
                                    () = tui_main.output.push(err_line);
                                    () = tui_main.scroll_to_bottom();
                                }
                            }
                        }
                    }
                }
                **dirty = true;
            }
            _ => (),
        }
    }
}

fn handle_output_area_input(
    mut messages: MessageReader<KeyMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
) {
    use crossterm::event::{KeyCode, KeyEvent};

    for message in messages.read() {
        let KeyEvent { code, .. } = &**message;

        if let KeyCode::Char(' ') = code {
            () = tui_main.scroll_page_down();
            **dirty = true;
        }
    }
}

fn handle_mouse_input(
    mut messages: MessageReader<MouseMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
) {
    use crossterm::event::{MouseEvent, MouseEventKind};

    for message in messages.read() {
        let MouseEvent { kind, .. } = &**message;
        match kind {
            MouseEventKind::ScrollUp => {
                () = tui_main.scroll_up();
                **dirty = true;
            }
            MouseEventKind::ScrollDown => {
                () = tui_main.scroll_down();
                **dirty = true;
            }
            _ => (),
        }
    }
}

fn draw_scene_system(
    mut context: ResMut<RatatuiContext>,
    mut tui: NonSendMut<TuiMain>,
    time: Res<Time<()>>,
    mut cursor_timer: Local<Option<Timer>>,
    mut dirty: ResMut<RenderNeeded>,
    token_usage: Option<Res<CodingAgentTotalTokenUsage>>,
) -> bevy::ecs::error::Result {
    let cursor_timer = cursor_timer.get_or_insert(Timer::new(
        Duration::from_millis(CURSOR_BLINK_INTERVAL_MS),
        TimerMode::Repeating,
    ));
    _ = cursor_timer.tick(time.delta());
    let TuiMain { show_cursor, .. } = &mut *tui;

    if cursor_timer.just_finished() {
        *show_cursor ^= true;
        **dirty = true;
    }

    if **dirty {
        _ = context.draw(|frame| {
            () = tui.draw(frame, token_usage.as_deref());
        })?;
    }

    **dirty = false;

    Ok(())
}

pub fn plugin(app: &mut App) {
    _ = app
        .init_non_send_resource::<TuiMain>()
        .init_state::<TuiMainFocus>()
        .add_systems(
            PreUpdate,
            (
                handle_global_input,
                handle_mouse_input,
                handle_input_area_input.run_if(in_state(TuiMainFocus::InputArea)),
                handle_output_area_input.run_if(in_state(TuiMainFocus::OutputArea)),
            )
                .chain(),
        )
        .add_systems(Update, draw_scene_system);
}
