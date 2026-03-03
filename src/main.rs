#![windows_subsystem = "windows"]

mod plugins;

use {
    crate::plugins::{tokio_plugin, tui_plugin},
    bevy::{
        MinimalPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin, Update},
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageId, MessageWriter},
            resource::Resource,
            schedule::ScheduleConfigTupleMarker,
            system::{IsFunctionSystem, Local},
        },
        input::ButtonInput,
        time::{Time, Timer, TimerMode},
    },
    bevy_ratatui::RatatuiContext,
    ratatui::{
        CompletedFrame, Frame,
        layout::{Constraint, Layout, Rect},
        style::Style,
        widgets::{Block, Paragraph},
    },
    std::time::Duration,
};

const FRAMES_PER_SECOND: f32 = 30.0;

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
            character_index: text.chars().count(),
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
    input: Res<'_, ButtonInput<bevy::input::keyboard::KeyCode>>,
    mut exit: MessageWriter<'_, AppExit>,
) {
    use bevy::input::keyboard::KeyCode;

    () = input
        .get_just_pressed()
        .for_each::<_>(|key_code: &KeyCode| {
            if key_code == &KeyCode::Escape {
                let _: MessageId<AppExit> = exit.write_default();
            }
        })
}

fn draw_scene_system(
    mut context: ResMut<'_, RatatuiContext>,
    root: Res<'_, Main>,
    time: Res<'_, Time<()>>,
    mut timer: Local<'_, Option<Timer>>,
    mut show_cursor: Local<'_, bool>,
) -> bevy::ecs::error::Result {
    let timer = timer.get_or_insert(Timer::new(Duration::from_millis(864), TimerMode::Repeating));
    let _: &Timer = timer.tick(time.delta());

    if timer.just_finished() {
        *show_cursor ^= true;
    }

    let _: CompletedFrame<'_> = context.draw::<_>(|frame: &mut Frame<'_>| {
        () = (*root).draw(frame, *show_cursor);
    })?;

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
        .init_resource::<Main>()
        .add_systems::<(
            ScheduleConfigTupleMarker,
            (
                IsFunctionSystem,
                fn(
                    _, // Res<'_, ButtonInput<bevy::input::keyboard::KeyCode>>
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
                ) -> bevy::ecs::error::Result,
            ),
        )>(Update, (hotkeys, draw_scene_system));

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
