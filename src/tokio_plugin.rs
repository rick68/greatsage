use {
    bevy::{
        app::{App, AppExit, PostUpdate, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            message::{MessageId, MessageReader},
            resource::Resource,
            system::IsFunctionSystem,
        },
        prelude::{Deref, DerefMut},
        tasks::futures_lite::StreamExt,
    },
    bevy_ratatui::crossterm,
    bevy_tokio_tasks::{MainThreadContext, TaskContext, TokioTasksPlugin, TokioTasksRuntime},
    signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM},
    signal_hook_tokio::{Signals, SignalsInfo},
    std::sync::Arc,
    tokio::{io, task::JoinHandle},
    tokio_util::sync::CancellationToken,
};

const SIGNALS: &[i32] = &[SIGHUP, SIGINT, SIGQUIT, SIGTERM];

#[derive(Default, Deref, DerefMut, Resource)]
struct AppCancelToken(Arc<CancellationToken>);

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
                                ctx.world.write_message_default::<AppExit>();
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

pub(super) fn plugin(app: &mut App) {
    let _: &mut App = app
        .add_plugins::<_>(TokioTasksPlugin::default())
        .insert_resource::<AppCancelToken>(AppCancelToken::default())
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
}
