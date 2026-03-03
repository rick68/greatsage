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
            system::IsFunctionSystem,
        },
        input::ButtonInput,
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

#[derive(Resource)]
struct Main {
    input: String,
}

impl Default for Main {
    fn default() -> Self {
        Self {
            // input: String::new(),
            input: String::from("coi le munje"),
        }
    }
}

impl Main {
    fn draw(&self, frame: &mut Frame<'_>) {
        let vertical: Layout = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]);
        let [_, input_area]: [Rect; 2] = vertical.areas::<2>(frame.area());

        let input: Paragraph<'_> = Paragraph::new::<&str>(self.input.as_str())
            .style::<Style>(Style::default())
            .block(Block::bordered());
        () = frame.render_widget::<_>(input, input_area);
    }
}

fn draw_scene_system(
    mut context: ResMut<'_, RatatuiContext>,
    root: Res<'_, Main>,
) -> bevy::ecs::error::Result {
    let _: CompletedFrame<'_> = context.draw::<_>(|frame: &mut Frame<'_>| {
        () = (*root).draw(frame);
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
                ) -> bevy::ecs::error::Result,
            ),
        )>(Update, (hotkeys, draw_scene_system));

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
