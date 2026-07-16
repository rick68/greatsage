use {
    crate::tokio::AppCancelToken,
    bevy::{
        app::{App, PreUpdate, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            message::{Message, MessageWriter},
            resource::Resource,
            system::Commands,
        },
        prelude::Deref,
    },
    bevy_ratatui::crossterm::event::{self, KeyEvent},
    bevy_tokio_tasks::TokioTasksRuntime,
    crossbeam_channel::{Receiver, unbounded},
    std::time::Duration,
};

/// Max wait for `event::poll` between cancel checks (not `thread::sleep`).
const STDIN_POLL_TIMEOUT_MS: u64 = 100;

#[derive(Message)]
pub struct StdinKeyMessage(pub KeyEvent);

#[derive(Deref, Resource)]
struct StreamReceiver(Receiver<KeyEvent>);

fn setup(
    mut commands: Commands,
    app_cancel: Res<AppCancelToken>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
) {
    let (tx, rx) = unbounded::<KeyEvent>();
    commands.insert_resource(StreamReceiver(rx));

    let app_cancel = app_cancel.clone();

    // Holds `AppCancelToken` until the blocking poll loop exits, then drops it.
    // Must complete on cancel so `shutdown_tokio_on_exit` can finish its barrier.
    tokio_runtime.spawn_background_task(|_ctx| async move {
        let join = tokio::task::spawn_blocking(move || {
            let timeout = Duration::from_millis(STDIN_POLL_TIMEOUT_MS);
            loop {
                if app_cancel.is_cancelled() {
                    break;
                }
                match event::poll(timeout) {
                    Ok(true) => {
                        // Re-check after poll: AppExit may have cancelled while waiting.
                        if app_cancel.is_cancelled() {
                            break;
                        }
                        match event::read() {
                            Ok(event::Event::Key(key)) => {
                                // Receiver gone (teardown) → leave so Arc can drop.
                                if tx.send(key).is_err() {
                                    break;
                                }
                            }
                            Ok(_) => {}
                            Err(_) => break,
                        }
                    }
                    Ok(false) => {}
                    Err(_) => break,
                }
            }
            // Arc dropped here when `app_cancel` goes out of scope.
        });
        // Await so this task does not finish before the blocking loop drops the token.
        let _ = join.await;
    });
}

fn forward_stdin_input_to_messsage(
    stdin_keys: ResMut<StreamReceiver>,
    mut messages: MessageWriter<StdinKeyMessage>,
) {
    while let Ok(key) = stdin_keys.try_recv() {
        messages.write(StdinKeyMessage(key));
    }
}

pub(crate) fn stdin_plugin(app: &mut App) {
    app.add_message::<StdinKeyMessage>()
        .add_systems(Startup, setup)
        .add_systems(PreUpdate, forward_stdin_input_to_messsage);
}
