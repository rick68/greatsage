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

    tokio_runtime.spawn_background_task(|_ctx| async move {
        let app_cancel = app_cancel.clone();

        tokio::task::spawn_blocking(move || {
            let timeout = Duration::from_millis(STDIN_POLL_TIMEOUT_MS);
            while !app_cancel.is_cancelled() {
                if event::poll(timeout).expect("Failed to poll stdin") {
                    let evt = event::read().expect("Failed to read stdin event");
                    if let event::Event::Key(key) = evt {
                        tx.send(key).expect("Failed to transmit key event");
                    }
                }
            }
        });
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
