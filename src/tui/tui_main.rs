use {
    super::RenderNeeded,
    crate::agents::CodingAgentPromptChannel,
    bevy::{
        app::{App, AppExit, PreUpdate, Update},
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            message::{MessageId, MessageReader, MessageWriter},
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
        style::Style,
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
const PROMPT_SUFFIX_LENGTH: usize = 5;

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
        let next: u8 = ((*self as usize + 1) % Self::COUNT) as u8;
        TuiMainFocus::from_repr(next).inspect(|item| *self = *item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (Self::COUNT, Some(Self::COUNT))
    }
}

impl ExactSizeIterator for TuiMainFocus {}

impl DoubleEndedIterator for TuiMainFocus {
    fn next_back(&mut self) -> Option<Self::Item> {
        let next: u8 = if (*self as u8) == 0 {
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

    fn draw(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Min(3), Constraint::Length(3)]);
        let area = frame.area();
        let [output_area, input_area] = vertical.areas::<2>(area);
        let chunks = vertical.split(area);

        self.output_area = output_area;

        let text = &self.output;
        let output = Paragraph::new(text.clone())
            .style(Style::default())
            .block(Block::bordered().title::<&str>("Output"))
            .scroll((self.vertical_scroll as u16, 0));
        self.vertical_scroll_state = self
            .vertical_scroll_state
            .content_length(self.max_scroll())
            .position(self.vertical_scroll);

        frame.render_widget(output, output_area);
        frame.render_stateful_widget::<Scrollbar>(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            chunks[0],
            &mut self.vertical_scroll_state,
        );

        let input = Paragraph::new(format!("{PROMPT_PREFIX}{}", self.input))
            .style(Style::default())
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, input_area);

        if self.show_cursor && self.focused == TuiMainFocus::InputArea {
            frame.set_cursor_position((
                input_area.left() + (self.character_index + PROMPT_SUFFIX_LENGTH) as u16 + 1,
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
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

    for message in messages.read() {
        let KeyEvent { code, kind, .. } = &**message;

        match code {
            KeyCode::Tab => {
                let TuiMain { focused, .. } = tui_main.as_mut();
                let next: TuiMainFocus = focused.next().unwrap();
                next_tui_main_focus.set(next);
                next_tui_main_focus.set(next);
                **dirty = true;
            }
            KeyCode::Esc => {
                let _: MessageId<AppExit> = exit.write_default();
            }
            KeyCode::Up if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat => {
                tui_main.scroll_up();
                **dirty = true;
            }
            KeyCode::Down if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat => {
                tui_main.scroll_down();
                **dirty = true;
            }
            KeyCode::PageUp => {
                tui_main.scroll_page_up();
                **dirty = true;
            }
            KeyCode::PageDown => {
                tui_main.scroll_page_down();
                **dirty = true;
            }
            KeyCode::Home => {
                tui_main.scroll_to_top();
                **dirty = true;
            }
            KeyCode::End => {
                tui_main.scroll_to_bottom();
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
                tui_main.input.push(*c);
                if let Some(width) = UnicodeWidthChar::width(*c) {
                    tui_main.character_index = tui_main.character_index.saturating_add(width);
                }
                **dirty = true;
            }
            KeyCode::Backspace if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat => {
                if let Some(c) = tui_main.input.pop()
                    && let Some(width) = UnicodeWidthChar::width(c)
                {
                    tui_main.character_index -= width;
                }
                **dirty = true;
            }
            KeyCode::Enter if kind == &KeyEventKind::Press => {
                if !tui_main.input.is_empty() {
                    let input = tui_main.input.clone();

                    match input.as_str() {
                        "/exit" | "/quit" => {
                            exit.write_default();
                        }
                        _ => (),
                    }

                    tui_main.output.push(Line::raw(input.clone()));
                    tui_main.input.clear();
                    tui_main.character_index = 0;
                    tui_main.scroll_to_bottom();

                    channel.sender.send(input).unwrap();
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
            tui_main.scroll_page_down();
            **dirty = true;
        }
    }
}

fn draw_scene_system(
    mut context: ResMut<RatatuiContext>,
    mut tui: NonSendMut<TuiMain>,
    time: Res<Time>,
    mut cursor_timer: Local<Option<Timer>>,
    mut dirty: ResMut<RenderNeeded>,
) -> bevy::ecs::error::Result {
    let cursor_timer = cursor_timer.get_or_insert(Timer::new(
        Duration::from_millis(CURSOR_BLINK_INTERVAL_MS),
        TimerMode::Repeating,
    ));
    cursor_timer.tick(time.delta());
    let TuiMain { show_cursor, .. } = &mut *tui;

    if cursor_timer.just_finished() {
        *show_cursor ^= true;
        **dirty = true;
    }

    if **dirty {
        context.draw(|frame: &mut Frame| {
             tui.draw(frame);
        })?;
    }

    **dirty = false;

    Ok(())
}

pub fn plugin(app: &mut App) {
    app.init_non_send_resource::<TuiMain>()
        .init_state::<TuiMainFocus>()
        .add_systems(
            PreUpdate,
            (
                handle_global_input,
                handle_input_area_input.run_if(in_state(TuiMainFocus::InputArea)),
                handle_output_area_input.run_if(in_state(TuiMainFocus::OutputArea)),
            )
                .chain(),
        )
        .add_systems(Update, draw_scene_system);
}
