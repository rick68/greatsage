use {
    super::RenderNeeded,
    bevy::{
        app::{App, AppExit, PreUpdate, Update},
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageId, MessageReader, MessageWriter},
            resource::Resource,
            system::{IsFunctionSystem, Local},
        },
        state::{
            app::AppExtStates,
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
        widgets::{Block, Paragraph},
    },
    std::{
        iter::{DoubleEndedIterator, ExactSizeIterator, Iterator},
        time::Duration,
    },
    strum::{EnumCount, FromRepr},
    unicode_width::UnicodeWidthChar,
};

const CURSOR_BLINK_INTERVAL_MS: u64 = 530;

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

#[derive(Resource)]
pub struct TuiMain {
    input: String,
    character_index: usize,
    output: Vec<String>,
    focused: TuiMainFocus,
}

impl Default for TuiMain {
    fn default() -> Self {
        Self {
            input: String::new(),
            character_index: 0,
            output: vec![],
            focused: TuiMainFocus::default(),
        }
    }
}

impl TuiMain {
    pub fn draw(&self, frame: &mut Frame<'_>, show_cursor: bool) {
        let vertical: Layout =
            Layout::vertical::<[Constraint; 2]>([Constraint::Min(3), Constraint::Length(3)]);
        let [output_area, input_area]: [Rect; 2] = vertical.areas::<2>(frame.area());

        let lines: Vec<Line<'_>> = self
            .output
            .iter()
            .map::<Line<'_>, fn(&String) -> Line<'_>>(|data: &String| -> Line<'_> {
                Line::raw(data)
            })
            .collect::<Vec<Line<'_>>>();

        let output: Paragraph<'_> = Paragraph::<'_>::new::<_>(lines)
            .style::<Style>(Style::default())
            .block(Block::bordered().title::<&str>("Output"));
        () = frame.render_widget::<Paragraph<'_>>(output, output_area);

        let input: Paragraph<'_> = Paragraph::<'_>::new::<&str>(self.input.as_str())
            .style::<Style>(Style::default())
            .block(Block::bordered().title::<&str>("Input"));
        () = frame.render_widget::<Paragraph<'_>>(input, input_area);

        if show_cursor && self.focused == TuiMainFocus::InputArea {
            () = frame.set_cursor_position::<(u16, u16)>((
                input_area.left() + self.character_index as u16 + 1,
                input_area.top() + 1,
            ));
        }
    }
}

fn hotkeys(
    mut messages: MessageReader<'_, '_, KeyMessage>,
    mut tui_main: ResMut<'_, TuiMain>,
    mut next_tui_main_focus: ResMut<'_, NextState<TuiMainFocus>>,
    mut dirty: ResMut<'_, RenderNeeded>,
    mut exit: MessageWriter<'_, AppExit>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

    let TuiMain {
        input,
        character_index,
        output,
        focused,
    } = tui_main.as_mut();

    for message in messages.read() {
        let KeyEvent { code, kind, .. } = &**message;

        match code {
            KeyCode::Esc => {
                let _: MessageId<AppExit> = exit.write_default();
            }
            KeyCode::Tab => {
                let next: TuiMainFocus = focused.next().unwrap();
                () = next_tui_main_focus.set(next);
            }
            _ => (),
        }

        match focused {
            TuiMainFocus::OutputArea => {}
            TuiMainFocus::InputArea => match code {
                KeyCode::Char(c)
                    if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat =>
                {
                    () = input.push(*c);
                    if let Some(width) = UnicodeWidthChar::width(*c) {
                        *character_index = character_index.saturating_add(width);
                    }
                }
                KeyCode::Backspace
                    if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat =>
                {
                    if let Some(c) = input.pop()
                        && let Some(width) = UnicodeWidthChar::width(c)
                    {
                        *character_index -= width;
                    }
                }
                KeyCode::Enter if kind == &KeyEventKind::Press => {
                    () = output.push(input.clone());
                    *input = String::new();
                    *character_index = 0;
                }
                _ => (),
            },
        }

        **dirty = true;
    }
}

fn draw_scene_system(
    mut context: ResMut<'_, RatatuiContext>,
    root: Res<'_, TuiMain>,
    time: Res<'_, Time<()>>,
    mut cursor_timer: Local<'_, Option<Timer>>,
    mut show_cursor: Local<'_, bool>,
    mut dirty: ResMut<'_, RenderNeeded>,
) -> bevy::ecs::error::Result {
    let cursor_timer: &mut Timer = cursor_timer.get_or_insert(Timer::new(
        Duration::from_millis(CURSOR_BLINK_INTERVAL_MS),
        TimerMode::Repeating,
    ));
    let _: &Timer = cursor_timer.tick(time.delta());

    if cursor_timer.just_finished() {
        *show_cursor ^= true;
        **dirty = true;
    }

    if **dirty {
        let _: CompletedFrame<'_> = context.draw::<_>(|frame: &mut Frame<'_>| {
            () = (*root).draw(frame, *show_cursor);
        })?;
    }

    **dirty = false;

    Ok(())
}

pub fn plugin(app: &mut bevy::app::App) {
    let _: &mut App = app
        .init_resource::<TuiMain>()
        .init_state::<TuiMainFocus>()
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // Res<'_, MessageReader<'_, '_, KeyMessage>>
                _, // ResMut<'_, TuiMain>
                _, // ResMut<'_, NextState<TuiMainFocus>>
                _, // ResMut<'_, RenderNeeded>
                _, // MessageWriter<'_, AppExit>
            ) -> (),
        )>(PreUpdate, hotkeys)
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // ResMut<'_, RatatuiContext>
                _, // Res<'_, Main>
                _, // Res<'_, Time<()>>
                _, // Local<'_, Option<Timer>>
                _, // Local<'_, bool>
                _, // ResMut<'_, RenderNeeded>
            ) -> bevy::ecs::error::Result,
        )>(Update, draw_scene_system);
}
