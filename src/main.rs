#![windows_subsystem = "windows"]

mod plugins;

use {
    crate::plugins::{tokio_plugin, tui_plugin},
    bevy::{
        MinimalPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin, Update},
        ecs::{
            change_detection::{NonSend, Res, ResMut},
            message::{MessageId, MessageWriter},
            schedule::ScheduleConfigTupleMarker,
            system::IsFunctionSystem,
        },
        input::ButtonInput,
    },
    bevy_ratatui::RatatuiContext,
    ratatui::{CompletedFrame, Frame, text::Text},
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

struct Main<'a> {
    text: Text<'a>,
}

impl Default for Main<'_> {
    fn default() -> Self {
        let mut text: Text<'_> = Text::default();
        () = text.push_line::<&str>("coi le munje");

        Self { text }
    }
}

impl<'a> Main<'a> {
    fn draw(&self, frame: &mut Frame<'a>) {
        () = frame.render_widget::<Text<'_>>(self.text.clone().centered(), frame.area());
    }
}

fn draw_scene_system(
    mut context: ResMut<'_, RatatuiContext>,
    root: NonSend<'_, Main<'_>>,
) -> bevy::ecs::error::Result {
    let _: CompletedFrame<'_> = context.draw::<_>(|frame: &mut Frame<'_>| {
        () = root.draw(frame);
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
        .init_non_send_resource::<Main<'_>>()
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
                    _, // NonSend<'_, Main<'_>>
                ) -> bevy::ecs::error::Result,
            ),
        )>(Update, (hotkeys, draw_scene_system));

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
