#[cfg(not(target_os = "windows"))]
use {
    bevy::tasks::futures_lite::StreamExt,
    bevy_ratatui::crossterm::{self, execute, terminal::LeaveAlternateScreen},
    signal_hook::consts::signal::{SIGHUP, SIGINT, SIGQUIT, SIGTERM},
    signal_hook_tokio::Signals,
    std::io::{self, IsTerminal},
};
use {
    bevy::{
        app::{App, AppExit, PostUpdate, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            message::MessageReader,
            resource::Resource,
        },
        prelude::Deref,
    },
    bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime},
    std::sync::Arc,
    tokio_util::sync::CancellationToken,
};

/// Signals that should Leave app (process exit).
///
/// **SIGINT is registered but must NOT AppExit**: in raw-mode REPL, Ctrl+C is
/// also delivered as a key event and is handled by state (shell interrupt /
/// agent abort / cancel line / second-press leave). If SIGINT always wrote
/// `AppExit`, LLM responses would kill the whole process on first Ctrl+C.
#[cfg(not(target_os = "windows"))]
const EXIT_SIGNALS: &[i32] = &[SIGHUP, SIGQUIT, SIGTERM];

#[cfg(not(target_os = "windows"))]
const HANDLED_SIGNALS: &[i32] = &[SIGHUP, SIGINT, SIGQUIT, SIGTERM];

#[derive(Default, Deref, Resource)]
pub struct AppCancelToken(Arc<CancellationToken>);

fn setup_signal_handles(runtime: ResMut<TokioTasksRuntime>, cancel: Res<AppCancelToken>) {
    let cancel = cancel.clone();
    #[allow(unused_mut, unused_variables)]
    runtime.spawn_background_task(|mut ctx| async move {
        #[cfg(not(target_os = "windows"))]
        {
            let mut signals = Signals::new(HANDLED_SIGNALS).unwrap();

            loop {
                tokio::select! {
                    Some(signal) = signals.next(), if cfg!(not(target_os = "windows")) => {
                        // Swallow SIGINT so the default terminate disposition does not
                        // kill the process; REPL/TUI key paths own Ctrl+C semantics.
                        if signal == SIGINT {
                            continue;
                        }
                        if !EXIT_SIGNALS.contains(&signal) {
                            continue;
                        }
                        () = ctx.run_on_main_thread(|ctx| {
                            let world = ctx.world;
                            if io::stdout().is_terminal() {
                                let _ = crossterm::terminal::disable_raw_mode();
                                let _ = execute!(io::stdout(), LeaveAlternateScreen);
                            }
                            world.write_message_default::<AppExit>();
                        }).await;
                    }
                    _ = cancel.cancelled() => break,
                    else => unreachable!(),
                }
            }
        }
        #[cfg(target_os = "windows")]
        {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    else => unreachable!(),
                }
            }
        }
    });
}

pub(crate) fn shutdown_tokio_on_exit(
    mut messages: MessageReader<AppExit>,
    mut cancel: Option<Res<AppCancelToken>>,
) {
    for _message in messages.read() {
        if let Some(cancel) = cancel.take()
            && !cancel.is_cancelled()
        {
            cancel.cancel();
            // **Intentional barrier (wait until strong_count == 1).**
            //
            // Clone holders (each must drop on cancel while the runtime still exists):
            // 1. signal_hook task — `tokio.rs` `setup_signal_handles` (`cancelled()` → break)
            // 2. agents waiter — `agents/mod.rs` setup (`cancelled()` → ())
            // 3. coding waiter — `agents/coding.rs` setup (`cancelled()` → ())
            // 4. stdin poll — `stdin.rs` spawn_blocking loop (`is_cancelled` between polls)
            //
            // Resource owns the last Arc. Do **not** timeout this barrier (runtime teardown
            // race). Main thread is not a Tokio worker: `yield_now` only lets OS schedule
            // runtime/blocking-pool threads that perform the actual leave+drop.
            // Holders must not need `run_on_main_thread` to drop (tick stops after exit).
            while Arc::strong_count(&cancel) != 1 {
                std::thread::yield_now();
            }
        }
    }
}

pub fn tokio_plugin(app: &mut App) {
    app.add_plugins(TokioTasksPlugin::default())
        .init_resource::<AppCancelToken>()
        .add_systems(Startup, setup_signal_handles)
        // After agent cancel token; before REPL `disable_raw_mode` (stdin must leave first).
        .add_systems(PostUpdate, shutdown_tokio_on_exit);
}
