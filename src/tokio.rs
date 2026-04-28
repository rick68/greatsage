use {
    bevy::{
        app::{App, AppExit, PostUpdate, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            message::MessageReader,
            resource::Resource,
        },
        prelude::Deref,
        tasks::futures_lite::StreamExt,
    },
    bevy_ratatui::{
        RatatuiContext,
        crossterm::{self, execute, terminal::LeaveAlternateScreen},
    },
    bevy_tokio_tasks::{MainThreadContext, TokioTasksPlugin, TokioTasksRuntime},
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
    _ = runtime.spawn_background_task(|mut ctx| async move {
        #[cfg(not(target_os = "windows"))]
        let mut signals = Signals::new(SIGNALS).unwrap();

        loop {
            tokio::select! {
                Some(_signal) = signals.next(), if cfg!(not(target_os = "windows")) => {
                    () = ctx.run_on_main_thread::<_, ()>(|ctx: MainThreadContext| {
                        // Only restore terminal state when TUI is active.
                        if ctx.world.get_resource::<RatatuiContext>().is_some() {
                            let _ = crossterm::terminal::disable_raw_mode();
                            let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
                        }
                        _ = ctx.world.write_message_default::<AppExit>();
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
    tui_ctx: Option<Res<RatatuiContext>>,
) {
    for _message in messages.read() {
        if let Some(cancel) = cancel.take()
            && !cancel.is_cancelled()
        {
            () = cancel.cancel();

            while Arc::strong_count(&cancel) != 1 {}

            // Only restore the terminal when the TUI was active.
            // In non-TUI (-p / pipe) mode this would emit LeaveAlternateScreen
            // to stdout, corrupting the streaming output.
            if tui_ctx.is_some() {
                () = ratatui::restore();
                _ = crossterm::terminal::disable_raw_mode();
            }
        }
    }
}

pub fn tokio_plugin(app: &mut App) {
    _ = app
        .add_plugins(TokioTasksPlugin::default())
        .init_resource::<AppCancelToken>()
        .add_systems(Startup, setup_signal_handles)
        .add_systems(PostUpdate, shutdown_tokio_on_exit);
}
