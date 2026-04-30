use {
    crate::tui::TuiMain,
    bevy::{
        app::{App, AppExit, PostUpdate, Startup},
        ecs::{
            change_detection::{NonSend, Res, ResMut},
            message::MessageReader,
            resource::Resource,
        },
        prelude::Deref,
        tasks::futures_lite::StreamExt,
    },
    bevy_ratatui::crossterm::{self, execute, terminal::LeaveAlternateScreen},
    bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime},
    signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM},
    signal_hook_tokio::Signals,
    std::sync::Arc,
    tokio_util::sync::CancellationToken,
};

const SIGNALS: &[i32] = &[SIGHUP, SIGINT, SIGQUIT, SIGTERM];

#[derive(Default, Deref, Resource)]
pub struct AppCancelToken(Arc<CancellationToken>);

fn setup_signal_handles(runtime: ResMut<TokioTasksRuntime>, cancel: Res<AppCancelToken>) {
    let cancel = cancel.clone();
    runtime.spawn_background_task(|mut ctx| async move {
        #[cfg(not(target_os = "windows"))]
        let mut signals = Signals::new(SIGNALS).unwrap();

        loop {
            tokio::select! {
                Some(_signal) = signals.next(), if cfg!(not(target_os = "windows")) => {
                    () = ctx.run_on_main_thread::<_, ()>(|ctx| {
                        let world = ctx.world;
                        if world.get_non_send_resource::<NonSend<TuiMain>>().is_some() {
                            _ = crossterm::terminal::disable_raw_mode();
                            _ = execute!(std::io::stdout(), LeaveAlternateScreen);
                        }
                        world.write_message_default::<AppExit>();
                    }).await;
                }
                _ = cancel.cancelled() => break,
                else => unreachable!(),
            }
        }
    });
}

fn shutdown_tokio_on_exit(
    mut messages: MessageReader<AppExit>,
    mut cancel: Option<Res<AppCancelToken>>,
    tui: Option<NonSend<TuiMain>>,
) {
    for _message in messages.read() {
        if let Some(cancel) = cancel.take()
            && !cancel.is_cancelled()
        {
            () = cancel.cancel();

            while Arc::strong_count(&cancel) != 1 {}

            if tui.is_some() {
                ratatui::restore();
                let _ =  crossterm::terminal::disable_raw_mode();
            }
        }
    }
}

pub fn tokio_plugin(app: &mut App) {
    let _: &mut App = app
        .add_plugins(TokioTasksPlugin::default())
        .init_resource::<AppCancelToken>()
        .add_systems(Startup, setup_signal_handles)
        .add_systems(PostUpdate, shutdown_tokio_on_exit);
}
