//! TUI auth actions: OperatorPanel + background device login (no Session ECS).

use {
    crate::{
        auth::{
            device::{DeviceLoginNotify, run_device_login_flow_notified},
            logout_provider,
            resolve::resolved_metadata,
            status_for,
        },
        config::Config,
        providers::Provider,
    },
    bevy::ecs::{change_detection::ResMut, resource::Resource, system::Res},
    bevy_tokio_tasks::TokioTasksRuntime,
    crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded},
};

/// Events from background auth tasks → TUI poll system.
#[derive(Clone, Debug)]
pub enum AuthUiEvent {
    /// Replace operator panel lines.
    PanelLines { title: String, lines: Vec<String> },
    /// Append one line to the open auth panel.
    PanelAppend(String),
    /// Status hint toast.
    StatusHint(String),
}

/// Channel resource for auth UI events.
#[derive(Resource)]
pub struct AuthUiChannel {
    tx: Sender<AuthUiEvent>,
    rx: Receiver<AuthUiEvent>,
}

impl Default for AuthUiChannel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self { tx, rx }
    }
}

impl AuthUiChannel {
    pub fn sender(&self) -> Sender<AuthUiEvent> {
        self.tx.clone()
    }

    pub fn try_recv(&self) -> Result<AuthUiEvent, TryRecvError> {
        self.rx.try_recv()
    }
}

struct ChannelNotify {
    tx: Sender<AuthUiEvent>,
}

impl DeviceLoginNotify for ChannelNotify {
    fn on_codes(&mut self, verification_url: &str, user_code: &str) {
        let _ = self.tx.send(AuthUiEvent::PanelLines {
            title: "auth login".into(),
            lines: vec![
                "OAuth device login (xAI)".into(),
                String::new(),
                format!("Open:  {verification_url}"),
                format!("Code:  {user_code}"),
                String::new(),
                String::from("Waiting for approval in the browser…"),
                String::from("(Esc closes this panel; login continues in background)"),
            ],
        });
    }

    fn on_line(&mut self, message: &str) {
        let _ = self.tx.send(AuthUiEvent::PanelAppend(message.to_owned()));
    }
}

/// Show non-secret auth status in the operator panel.
pub fn panel_auth_status(config: &Config) -> (String, Vec<String>) {
    let provider = config.get_provider().unwrap_or(Provider::Xai);
    let st = status_for(config, provider);
    (String::from("auth status"), st.format_lines())
}

/// Logout OAuth tokens; return panel content.
pub fn panel_auth_logout(config: &Config) -> (String, Vec<String>) {
    let provider = config.get_provider().unwrap_or(Provider::Xai);
    match logout_provider(provider) {
        Ok(()) => (
            String::from("auth logout"),
            vec![
                format!("Logged out OAuth tokens for `{provider}`."),
                String::from("API keys in env/config are unchanged."),
            ],
        ),
        Err(err) => (
            String::from("auth logout"),
            vec![format!("logout failed: {err}")],
        ),
    }
}

/// Drain [`AuthUiChannel`] into OperatorPanel / status hint each frame.
pub fn poll_auth_ui_system(channel: Res<AuthUiChannel>, mut state: ResMut<super::state::TuiState>) {
    loop {
        match channel.try_recv() {
            Ok(AuthUiEvent::PanelLines { title, lines }) => {
                () = state.open_operator_panel(title, lines);
            }
            Ok(AuthUiEvent::PanelAppend(line)) => {
                state.operator_panel.append_line(line);
            }
            Ok(AuthUiEvent::StatusHint(hint)) => {
                () = state.set_status_hint(hint);
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

/// Start device-code login on the app Tokio runtime; progress → [`AuthUiChannel`].
pub fn spawn_device_login(
    config: &Config,
    runtime: &TokioTasksRuntime,
    channel: &AuthUiChannel,
    open_browser: bool,
) {
    let provider = config.get_provider().unwrap_or(Provider::Xai);
    let Some(meta) = resolved_metadata(Some(config), provider) else {
        let _ = channel.sender().send(AuthUiEvent::PanelLines {
            title: "auth login".into(),
            lines: vec![
                format!("OAuth login not supported for `{provider}`."),
                String::from("Use an API key, or: greatsage login --help"),
            ],
        });
        return;
    };

    let _ = channel.sender().send(AuthUiEvent::PanelLines {
        title: "auth login".into(),
        lines: vec![
            format!("Starting device login for `{provider}`…"),
            String::from("Requesting device code…"),
        ],
    });
    let _ = channel
        .sender()
        .send(AuthUiEvent::StatusHint(String::from("auth login…")));

    let tx = channel.sender();
    let meta = meta.clone();

    runtime.spawn_background_task(move |_ctx| async move {
        let mut notify = ChannelNotify { tx: tx.clone() };
        match run_device_login_flow_notified(&meta, open_browser, &mut notify).await {
            Ok(_) => {
                let _ = tx.send(AuthUiEvent::PanelAppend(String::from(
                    "Tokens stored. Reinstall agent or restart session to pick them up.",
                )));
                let _ = tx.send(AuthUiEvent::StatusHint(String::from("auth: login ok")));
            }
            Err(err) => {
                let _ = tx.send(AuthUiEvent::PanelAppend(format!("login failed: {err}")));
                let _ = tx.send(AuthUiEvent::StatusHint(String::from("auth: login failed")));
            }
        }
    });
}
