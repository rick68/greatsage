#![windows_subsystem = "windows"]

mod plugins;

use {
    crate::plugins::{tokio_plugin, tui_plugin},
    bevy::{
        MinimalPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin, Update},
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageId, MessageReader, MessageWriter},
            resource::Resource,
            schedule::ScheduleConfigTupleMarker,
            system::{IsFunctionSystem, Local},
        },
        prelude::{Deref, DerefMut},
        time::{Time, Timer, TimerMode},
    },
    bevy_ratatui::{RatatuiContext, event::KeyMessage},
    ratatui::{
        CompletedFrame, Frame, crossterm,
        layout::{Constraint, Layout, Rect},
        style::Style,
        widgets::{Block, Paragraph},
    },
    std::time::Duration,
    unicode_width::{UnicodeWidthChar, UnicodeWidthStr},
};

const FRAMES_PER_SECOND: f32 = 30.0;
const CURSOR_BLINK_INTERVAL_MS: u64 = 530;

#[derive(Deref, DerefMut, Resource)]
struct RenderNeeded(bool);

impl Default for RenderNeeded {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource)]
struct Main {
    input: String,
    character_index: usize,
}

impl Default for Main {
    fn default() -> Self {
        let text: &str = "coi le munje";
        Self {
            input: String::from(text),
            character_index: UnicodeWidthStr::width(text),
        }
    }
}

impl Main {
    fn draw(&self, frame: &mut Frame<'_>, show_cursor: bool) {
        let vertical: Layout = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]);
        let [_, input_area]: [Rect; 2] = vertical.areas::<2>(frame.area());

        let input: Paragraph<'_> = Paragraph::new::<&str>(self.input.as_str())
            .style::<Style>(Style::default())
            .block(Block::bordered());
        () = frame.render_widget::<_>(input, input_area);

        if show_cursor {
            frame.set_cursor_position::<(u16, u16)>((
                input_area.left() + self.character_index as u16 + 1,
                input_area.top() + 1,
            ));
        }
    }
}

fn hotkeys(
    mut messages: MessageReader<'_, '_, KeyMessage>,
    mut root: ResMut<'_, Main>,
    mut dirty: ResMut<'_, RenderNeeded>,
    mut exit: MessageWriter<'_, AppExit>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

    let Main {
        input,
        character_index,
    } = root.as_mut();

    for message in messages.read() {
        let KeyEvent { code, kind, .. } = &**message;

        match code {
            KeyCode::Char(c) if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat => {
                () = input.push(*c);
                if let Some(width) = UnicodeWidthChar::width(*c) {
                    *character_index = character_index.saturating_add(width);
                }
            }
            KeyCode::Backspace if kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat => {
                if let Some(c) = input.pop()
                    && let Some(width) = UnicodeWidthChar::width(c)
                {
                    *character_index -= width;
                }
            }
            KeyCode::Esc => {
                let _: MessageId<AppExit> = exit.write_default();
            }
            _ => (),
        }

        **dirty = true;
    }
}

fn draw_scene_system(
    mut context: ResMut<'_, RatatuiContext>,
    root: Res<'_, Main>,
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

fn main() {
    let mut app: App = App::new();

    let _: &mut App = app
        .add_plugins::<(_, _, _, _)>((
            MinimalPlugins
                .set::<ScheduleRunnerPlugin>(ScheduleRunnerPlugin::run_loop(
                    Duration::from_secs_f32(FRAMES_PER_SECOND.recip()),
                ))
                .build(),
            tokio_plugin,
            tui_plugin,
        ))
        .init_resource::<RenderNeeded>()
        .init_resource::<Main>()
        .add_systems::<(
            ScheduleConfigTupleMarker,
            (
                IsFunctionSystem,
                fn(
                    _, // Res<'_, MessageReader<'_, '_, KeyMessage>>
                    _, // ResMut<'_, Main>
                    _, // ResMut<'_, RenderNeeded>
                    _, // MessageWriter<'_, AppExit>
                ) -> (),
            ),
            (
                IsFunctionSystem,
                fn(
                    _, // ResMut<'_, RatatuiContext>
                    _, // Res<'_, Main>
                    _, // Res<'_, Time<()>>
                    _, // Local<'_, Option<Timer>>
                    _, // Local<'_, bool>
                    _, // ResMut<'_, RenderNeeded>
                ) -> bevy::ecs::error::Result,
            ),
        )>(Update, (hotkeys, draw_scene_system));

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
