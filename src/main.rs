use {
    bevy::{
        MinimalPlugins,
        app::{App, AppExit, PluginGroup, PostUpdate, ScheduleRunnerPlugin, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageId, MessageReader},
            resource::Resource,
            system::IsFunctionSystem,
        },
        prelude::{Deref, DerefMut},
        tasks::futures_lite::StreamExt,
        utils::default,
    },
    bevy_ratatui::{RatatuiPlugins, crossterm},
    bevy_tokio_tasks::{
        MainThreadContext, TaskContext, TokioTasksPlugin, TokioTasksRuntime,
        tokio::task::JoinHandle,
    },
    signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM},
    signal_hook_tokio::{Signals, SignalsInfo},
    std::{sync::Arc, time::Duration},
    tokio::io,
    tokio_util::sync::CancellationToken,
};

const FRAMES_PER_SECOND: f32 = 30.0;
const SIGNALS: &[i32] = &[SIGHUP, SIGINT, SIGQUIT, SIGTERM];

fn setup_signal_handles(runtime: ResMut<'_, TokioTasksRuntime>, cancel: Res<'_, AppCancelToken>) {
    let cancel: Arc<CancellationToken> = cancel.clone();
    let _: JoinHandle<()> =
        runtime.spawn_background_task::<_, (), _>(|mut ctx: TaskContext| async move {
            #[cfg(not(target_os = "windows"))]
            let mut signals: SignalsInfo = Signals::new(SIGNALS).unwrap();

            loop {
                tokio::select! {
                    Some(_signal) = signals.next(), if cfg!(not(target_os = "windows")) => {
                        let _: io::Result<()> = crossterm::terminal::disable_raw_mode();

                        () = ctx.run_on_main_thread::<_, ()>(move |ctx: MainThreadContext<'_>| {
                            let _: Option<MessageId<AppExit>> =
                                ctx.world.write_message::<AppExit>(AppExit::Success);
                        }).await;
                    }
                    _ = cancel.cancelled() => {
                        break;
                    }
                    else => {
                        break;
                    },
                }
            }
        });
}

#[derive(Default, Deref, DerefMut, Resource)]
struct AppCancelToken(Arc<CancellationToken>);

fn shutdown_tokio_on_exit(
    mut messages: MessageReader<'_, '_, AppExit>,
    mut cancel: Option<Res<'_, AppCancelToken>>,
) {
    for _message in messages.read() {
        if let Some(cancel) = cancel.take()
            && !cancel.is_cancelled()
        {
            () = cancel.cancel();

            while Arc::strong_count(&cancel) != 1 {}

            let _ = ratatui::restore();
            let _: io::Result<()> = crossterm::terminal::disable_raw_mode();
        }
    }
}

fn main() {
    let mut app: App = App::new();

    let _: &mut App = app
        .insert_resource(AppCancelToken::default())
        .add_plugins::<(_, _, _, _)>((
            MinimalPlugins
                .set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                    FRAMES_PER_SECOND.recip(),
                )))
                .build(),
            TokioTasksPlugin::default(),
            RatatuiPlugins {
                enable_input_forwarding: true,
                ..default::<RatatuiPlugins>()
            },
        ))
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // ResMut<'_, TokioTasksRuntime>
                _, // Res<'_, AppCancelToken>
            ) -> (),
        )>(Startup, setup_signal_handles)
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // MessageReader<'_, '_, AppExit>
                _, // Option<Res<'_, AppCancelToken>>
            ) -> (),
        )>(PostUpdate, shutdown_tokio_on_exit);

    if let AppExit::Error(code) = app.run() {
        () = std::process::exit(code.get() as i32);
    }
}
