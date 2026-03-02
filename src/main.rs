#![windows_subsystem = "windows"]

mod plugins;

use {
    crate::plugins::tokio_plugin,
    bevy::{
        MinimalPlugins,
        app::{App, AppExit, PluginGroup, ScheduleRunnerPlugin, Update},
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageId, MessageWriter},
            schedule::ScheduleConfigTupleMarker,
            system::IsFunctionSystem,
        },
        input::{ButtonInput, keyboard::KeyCode},
        utils::default,
    },
    bevy_ratatui::{RatatuiContext, RatatuiPlugins},
    ratatui::{CompletedFrame, Frame, text::Text},
    std::time::Duration,
};

const FRAMES_PER_SECOND: f32 = 30.0;

fn draw_scene_system(mut context: ResMut<'_, RatatuiContext>) -> bevy::ecs::error::Result {
    let mut text: Text<'_> = Text::default();

    let _: CompletedFrame<'_> = context.draw::<_>(|frame: &mut Frame<'_>| {
        () = text.push_line::<&str>("coi le munje");
        () = frame.render_widget::<Text<'_>>(text.centered(), frame.area())
    })?;

    Ok(())
}

fn hotkeys(
    input: Res<'_, ButtonInput<bevy::input::keyboard::KeyCode>>,
    mut exit: MessageWriter<'_, AppExit>,
) {
    () = input
        .get_just_pressed()
        .for_each::<_>(|key_code: &bevy::input::keyboard::KeyCode| {
            if key_code == &KeyCode::Escape {
                let _: MessageId<AppExit> = exit.write_default();
            }
        })
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
            tokio_plugin::plugin,
            RatatuiPlugins {
                enable_mouse_capture: true,
                enable_input_forwarding: true,
                ..default::<RatatuiPlugins>()
            },
        ))
        .add_systems::<(
            ScheduleConfigTupleMarker,
            (
                IsFunctionSystem,
                fn(
                    _, // ResMut<'_, RatatuiContext>
                ) -> bevy::ecs::error::Result,
            ),
            (
                IsFunctionSystem,
                fn(
                    _, // Res<'_, ButtonInput<bevy::input::keyboard::KeyCode>>
                    _, // MessageWriter<'_, AppExit>
                ) -> (),
            ),
        )>(Update, (draw_scene_system, hotkeys));

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
