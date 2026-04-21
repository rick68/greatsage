use {
    super::RenderNeeded,
    crate::agents::CodingAgentPromptChannel,
    bevy::{
        app::{App, AppExit, PreUpdate, Update},
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            message::{MessageId, MessageReader, MessageWriter},
            schedule::IntoScheduleConfigs,
            system::{IsFunctionSystem, Local},
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
        CompletedFrame, Frame,
        layout::{Constraint, Layout, Rect},
        style::Style,
        text::Line,
        widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    },
    std::{
        iter::{DoubleEndedIterator, ExactSizeIterator, Iterator},
        rc::Rc,
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
        TuiMainFocus::from_repr(next).inspect::<_>(|item: &TuiMainFocus| *self = *item)
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
        TuiMainFocus::from_repr(next).inspect::<_>(|item: &TuiMainFocus| *self = *item)
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

    fn draw(&mut self, frame: &mut Frame<'_>) {
        let vertical: Layout =
            Layout::vertical::<[Constraint; 2]>([Constraint::Min(3), Constraint::Length(3)]);
        let area: Rect = frame.area();
        let [output_area, input_area]: [Rect; 2] = vertical.areas::<2>(area);
        let chunks: Rc<[Rect]> = vertical.split(area);

        self.output_area = output_area;

        let text: &Vec<Line<'_>> = &self.output;
        let output: Paragraph<'_> = Paragraph::<'_>::new::<Vec<Line<'_>>>(text.clone())
            .style::<Style>(Style::default())
            .block(Block::<'_>::bordered().title::<&str>("Output"))
            .scroll((self.vertical_scroll as u16, 0));
        self.vertical_scroll_state = self
            .vertical_scroll_state
            .content_length(self.max_scroll())
            .position(self.vertical_scroll);

        () = frame.render_widget::<Paragraph<'_>>(output, output_area);
        () = frame.render_stateful_widget::<Scrollbar>(
            Scrollbar::<'_>::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            chunks[0],
            &mut self.vertical_scroll_state,
        );

        let input: Paragraph<'_> =
            Paragraph::<'_>::new::<String>(format!("{}{}", PROMPT_PREFIX, self.input))
                .style::<Style>(Style::default())
                .block(Block::<'_>::bordered().title::<&str>("Input"));
        () = frame.render_widget::<Paragraph<'_>>(input, input_area);

        if self.show_cursor && self.focused == TuiMainFocus::InputArea {
            () = frame.set_cursor_position::<(u16, u16)>((
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
    mut messages: MessageReader<'_, '_, KeyMessage>,
    mut tui_main: NonSendMut<'_, TuiMain<'_>>,
    mut dirty: ResMut<'_, RenderNeeded>,
    mut next_tui_main_focus: ResMut<'_, NextState<TuiMainFocus>>,
    mut exit: MessageWriter<'_, AppExit>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

    for message in messages.read() {
        let KeyEvent { code, kind, .. } = &**message;

        match code {
            KeyCode::Tab => {
                let TuiMain::<'_> { focused, .. } = tui_main.as_mut();
                let next: TuiMainFocus = focused.next().unwrap();
                () = next_tui_main_focus.set(next);
                **dirty = true;
            }
            KeyCode::Esc => {
                let _: MessageId<AppExit> = exit.write_default();
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
    mut messages: MessageReader<'_, '_, KeyMessage>,
    mut tui_main: NonSendMut<'_, TuiMain<'_>>,
    mut dirty: ResMut<'_, RenderNeeded>,
    mut exit: MessageWriter<'_, AppExit>,
    channel: Res<'_, CodingAgentPromptChannel>,
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
                    tui_main.character_index -= width;
                }
                **dirty = true;
            }
            KeyCode::Enter if kind == &KeyEventKind::Press => {
                if !tui_main.input.is_empty() {
                    let input: String = tui_main.input.clone();

                    match input.as_str() {
                        "/exit" | "/quit" => {
                            let _: MessageId<AppExit> = exit.write_default();
                        }
                        _ => (),
                    }

                    () = tui_main
                        .output
                        .push(Line::<'_>::raw::<String>(input.clone()));
                    () = tui_main.input.clear();
                    tui_main.character_index = 0;
                    () = tui_main.scroll_to_bottom();

                    () = channel.sender.send(input).unwrap();
                }
                **dirty = true;
            }
            _ => (),
        }
    }
}

fn handle_output_area_input(
    mut messages: MessageReader<'_, '_, KeyMessage>,
    mut tui_main: NonSendMut<'_, TuiMain<'_>>,
    mut dirty: ResMut<'_, RenderNeeded>,
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
    mut context: ResMut<'_, RatatuiContext>,
    mut tui: NonSendMut<'_, TuiMain<'_>>,
    time: Res<'_, Time<()>>,
    mut cursor_timer: Local<'_, Option<Timer>>,
    mut dirty: ResMut<'_, RenderNeeded>,
) -> bevy::ecs::error::Result {
    let cursor_timer: &mut Timer = cursor_timer.get_or_insert(Timer::new(
        Duration::from_millis(CURSOR_BLINK_INTERVAL_MS),
        TimerMode::Repeating,
    ));
    let _: &Timer = cursor_timer.tick(time.delta());
    let TuiMain::<'_> { show_cursor, .. } = &mut *tui;

    if cursor_timer.just_finished() {
        *show_cursor ^= true;
        **dirty = true;
    }

    if **dirty {
        let _: CompletedFrame<'_> = context.draw::<_>(|frame: &mut Frame<'_>| {
            () = tui.draw(frame);
        })?;
    }

    **dirty = false;

    Ok(())
}

pub fn plugin(app: &mut App) {
    let _: &mut App = app
        .init_non_send_resource::<TuiMain<'_>>()
        .init_state::<TuiMainFocus>()
        .add_systems::<()>(
            PreUpdate,
            (
                handle_global_input,
                handle_input_area_input.run_if::<(
                    IsFunctionSystem,
                    fn(
                        Option<
                            _, // Res<'_, State<TuiMainFocus>>
                        >,
                    ) -> bool,
                )>(in_state::<TuiMainFocus>(
                    TuiMainFocus::InputArea,
                )),
                handle_output_area_input.run_if::<(
                    IsFunctionSystem,
                    fn(
                        Option<
                            _, // Res<'_, State<TuiMainFocus>>
                        >,
                    ) -> bool,
                )>(in_state::<TuiMainFocus>(
                    TuiMainFocus::OutputArea,
                )),
            )
                .chain(),
        )
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // ResMut<'_, RatatuiContext>
                _, // NonSendMut<'_, TuiMain<'_>>
                _, // Res<'_, Time<()>>
                _, // Local<'_, Option<Timer>>
                _, // ResMut<'_, RenderNeeded>
            ) -> bevy::ecs::error::Result,
        )>(Update, draw_scene_system);
}
