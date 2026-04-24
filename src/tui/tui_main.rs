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
            state::{NextState, States},
        },
        time::{Time, Timer, TimerMode},
    },
    bevy_ratatui::{RatatuiContext, crossterm, event::KeyMessage},
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        style::{Style, Stylize},
        text::Line,
        widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    },
    std::{
        iter::{DoubleEndedIterator, ExactSizeIterator, Iterator},
        time::Duration,
    },
    strum::{EnumCount, FromRepr},
    unicode_width::UnicodeWidthChar,
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
    character_index: usize,
    pub output: Vec<Line<'a>>,
    output_area: Rect,
    show_cursor: bool,
    focused: TuiMainFocus,
    vertical_scroll: usize,
    vertical_scroll_state: ScrollbarState,
}

impl<'a> TuiMain<'a> {
    fn output_area_height(&self) -> usize {
        const BORDER: u16 = 2;
        self.output_area.height.saturating_sub(BORDER) as usize
    }

    fn max_scroll(&self) -> usize {
        self.output.len().saturating_sub(self.output_area_height())
    }

    fn draw(&mut self, frame: &mut Frame, token_usage: Option<&CodingAgentTotalTokenUsage>) {
        let vertical = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ]);
        let area = frame.area();
        let [output_area, status_area, input_area] = vertical.areas(area);
        self.output_area = output_area;

        let text = &self.output;
        let output = Paragraph::new(text.clone())
            .style(Style::default())
            .block(Block::bordered().title("Output"))
            .scroll((self.vertical_scroll as u16, 0));
        self.vertical_scroll_state = self
            .vertical_scroll_state
            .content_length(self.output.len())
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
                " 🎯 Input: {} | Output: {} | Total: {} | Cache Read: {} | Cache Write: {}",
                usage.input, usage.output, usage.total_tokens, usage.cache_read, usage.cache_write
            )
        } else {
            " 🎯 Token usage: Waiting for first response...".to_string()
        };
        let status = Paragraph::new(status_text)
            .style(Style::default().fg(ratatui::style::Color::Rgb(100, 150, 200)))
            .block(Block::bordered().title("Token Usage"));
        () = frame.render_widget(status, status_area);

        let input = Paragraph::new(format!("{PROMPT_PREFIX}{}", self.input))
            .style(Style::default())
            .block(Block::bordered().title("Input"));
        () = frame.render_widget::<Paragraph<'_>>(input, input_area);

        if self.show_cursor && self.focused == TuiMainFocus::InputArea {
            use unicode_width::UnicodeWidthStr;
            let prefix_width = UnicodeWidthStr::width(PROMPT_PREFIX);
            () = frame.set_cursor_position((
                input_area.left() + (self.character_index + prefix_width) as u16 + 1,
                input_area.top() + 1,
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
        if self.output.len() > self.output_area_height() {
            self.vertical_scroll = self.max_scroll();
        }
    }
}

fn handle_global_input(
    mut messages: MessageReader<KeyMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut next_tui_main_focus: ResMut<NextState<TuiMainFocus>>,
    mut exit: MessageWriter<AppExit>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    for message in messages.read() {
        let KeyEvent {
            code,
            kind,
            modifiers,
            ..
        } = &**message;

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
            KeyCode::Up if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat => {
                () = tui_main.scroll_up();
                **dirty = true;
            }
            KeyCode::Down if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat => {
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
            KeyCode::Home => {
                () = tui_main.scroll_to_top();
                **dirty = true;
            }
            KeyCode::End => {
                () = tui_main.scroll_to_bottom();
                **dirty = true;
            }
            _ => (),
        }
    }
}

fn handle_input_area_input(
    mut messages: MessageReader<KeyMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut exit: MessageWriter<AppExit>,
    channel: Res<CodingAgentPromptChannel>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

    for message in messages.read() {
        let KeyEvent { code, kind, .. } = &**message;

        match code {
            KeyCode::Char(c) if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat => {
                () = tui_main.input.push(*c);
                if let Some(width) = UnicodeWidthChar::width(*c) {
                    tui_main.character_index = tui_main.character_index.saturating_add(width);
                }
                **dirty = true;
            }
            KeyCode::Backspace if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat => {
                if let Some(c) = tui_main.input.pop()
                    && let Some(width) = UnicodeWidthChar::width(c)
                {
                    tui_main.character_index = tui_main.character_index.saturating_sub(width);
                }
                **dirty = true;
            }
            KeyCode::Enter if kind == &KeyEventKind::Press => {
                if !tui_main.input.is_empty() {
                    let input = tui_main.input.clone();

                    // Handle built‑in REPL commands before sending to LLM.
                    if input.trim_start().starts_with("git ") {
                        // Parse simple git subcommands.
                        let output_line = if input.trim() == "git stage" {
                            match crate::git::stage_all() {
                                Ok(_) => Line::from("✅ Staged all changes").green(),
                                Err(e) => Line::from(format!("❌ git stage failed: {e}")).red(),
                            }
                        } else if input.trim_start().starts_with("git commit") {
                            // Expect format: git commit -m <msg>
                            // Find -m and extract following message.
                            let parts: Vec<&str> = input.splitn(4, ' ').collect();
                            // parts[0]=git, parts[1]=commit, parts[2]=maybe -m, parts[3]=msg
                            let msg_opt = parts.iter().skip(2).find_map(|p| {
                                if p.starts_with("-m") {
                                    // Remove leading -m and possible surrounding quotes
                                    let msg = p.trim_start_matches("-m");
                                    // If message after -m in same token, strip leading whitespace
                                    if msg.is_empty() {
                                        None
                                    } else {
                                        Some(msg.trim_matches('"').trim_matches('\'').trim())
                                    }
                                } else {
                                    None
                                }
                            });
                            // If message not captured, try to get the next token after -m
                            let msg = if let Some(m) = msg_opt {
                                m.to_string()
                            } else {
                                // fallback: take everything after "-m " substring
                                if let Some(idx) = input.find("-m ") {
                                    input[idx + 3..]
                                        .trim()
                                        .trim_matches('"')
                                        .trim_matches('\'')
                                        .to_string()
                                } else {
                                    String::new()
                                }
                            };
                            if msg.is_empty() {
                                Line::from("❌ git commit missing -m message").red()
                            } else {
                                match crate::git::commit(&msg) {
                                    Ok(_) => Line::from(format!("✅ Commit: {msg}")).green(),
                                    Err(e) => {
                                        Line::from(format!("❌ git commit failed: {e}")).red()
                                    }
                                }
                            }
                        } else if input.trim() == "git revert" {
                            match crate::git::revert_last() {
                                Ok(_) => Line::from("✅ Reverted last commit").green(),
                                Err(e) => Line::from(format!("❌ git revert failed: {e}")).red(),
                            }
                        } else {
                            Line::from("❌ Unknown git command").red()
                        };
                        () = tui_main.output.push(output_line);
                        () = tui_main.input.clear();
                        tui_main.character_index = 0;
                        () = tui_main.scroll_to_bottom();
                    } else {
                        match input.as_str() {
                            "/exit" | "/quit" => {
                                _ = exit.write_default();
                                () = tui_main.input.clear();
                                tui_main.character_index = 0;
                            }
                            _ => {
                                () = tui_main.output.push(Line::raw(input.clone()));
                                () = tui_main.input.clear();
                                tui_main.character_index = 0;
                                () = tui_main.scroll_to_bottom();
                                () = channel.sender.send(input).unwrap();
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
                handle_input_area_input.run_if(in_state::<TuiMainFocus>(TuiMainFocus::InputArea)),
                handle_output_area_input.run_if(in_state::<TuiMainFocus>(TuiMainFocus::OutputArea)),
            )
                .chain(),
        )
        .add_systems(Update, draw_scene_system);
}
