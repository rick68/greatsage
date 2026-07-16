//! OAuth 2.0 device authorization grant (+ PKCE S256 when required).

use {
    super::{
        flow::FlowError,
        pkce::generate_pkce_session,
        providers::OauthClientMetadata,
        store::{StoredTokens, save_tokens},
    },
    serde::Deserialize,
    std::time::Duration,
    tokio::time::{sleep, timeout},
};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(5);
const DEFAULT_DEVICE_TIMEOUT: Duration = Duration::from_secs(600);

/// Operator-facing progress (never secrets). CLI uses eprint; TUI uses a channel.
pub trait DeviceLoginNotify: Send {
    fn on_codes(&mut self, verification_url: &str, user_code: &str);
    fn on_line(&mut self, message: &str);
}

/// Default notifier for CLI / setup (stderr).
pub struct EprintNotify;

impl DeviceLoginNotify for EprintNotify {
    fn on_codes(&mut self, verification_url: &str, user_code: &str) {
        eprintln!("Device login:");
        eprintln!("  Open:  {verification_url}");
        eprintln!("  Code:  {user_code}");
        eprintln!("Waiting for approval… (Ctrl+C aborts the process)");
    }

    fn on_line(&mut self, message: &str) {
        eprintln!("{message}");
    }
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    /// Present on success; optional so error / rate-limit bodies can still parse.
    #[serde(default)]
    device_code: Option<String>,
    #[serde(default)]
    user_code: Option<String>,
    verification_uri: Option<String>,
    verification_uri_complete: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    interval: Option<u64>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
    /// Some gateways put a human message here on 429.
    #[serde(default)]
    message: Option<String>,
}

fn truncate_body(body: &str, max: usize) -> String {
    let t = body.trim();
    if t.chars().count() <= max {
        t.to_owned()
    } else {
        let head: String = t.chars().take(max).collect();
        format!("{head}…")
    }
}

fn device_request_error(status: reqwest::StatusCode, body: &str) -> FlowError {
    let parsed: Result<DeviceCodeResponse, _> = serde_json::from_str(body);
    let detail = parsed
        .ok()
        .and_then(|d| d.error_description.or(d.message).or(d.error))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(truncate_body(body, 160));

    if status.as_u16() == 429 {
        return FlowError::Message(format!(
            "xAI rate-limited device login (HTTP 429): {detail}\n\
             Wait a few minutes and try again, or use API key:\n\
               export XAI_API_KEY=…\n\
               greatsage setup  # choose API key, or set auth.xai.mode = \"api_key\""
        ));
    }
    FlowError::TokenEndpoint(format!(
        "device authorization failed (HTTP {status}): {detail}"
    ))
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// Run device-code login with a custom notifier, then save store.
pub async fn run_device_login_flow_notified(
    meta: &OauthClientMetadata,
    open_browser: bool,
    notify: &mut dyn DeviceLoginNotify,
) -> Result<StoredTokens, FlowError> {
    let device_url = meta
        .device_authorization_endpoint
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or(FlowError::Message(
            "device_authorization_endpoint not configured for this provider".into(),
        ))?;

    let pkce = generate_pkce_session();
    let client = reqwest::Client::new();

    let resp = client
        .post(device_url)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", meta.client_id.as_str()),
            ("scope", meta.scopes.as_str()),
            ("code_challenge", pkce.challenge.as_str()),
            ("code_challenge_method", "S256"),
        ])
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        return Err(device_request_error(status, &body));
    }

    let device: DeviceCodeResponse = serde_json::from_str(&body).map_err(|err| {
        FlowError::TokenEndpoint(format!(
            "invalid device-code response (HTTP {status}): {err}; body={}",
            truncate_body(&body, 160)
        ))
    })?;

    if let Some(err) = device.error {
        let detail = device.error_description.or(device.message).unwrap_or(err);
        return Err(FlowError::AuthorizeError(detail));
    }

    let device_code =
        device
            .device_code
            .filter(|s| !s.trim().is_empty())
            .ok_or(FlowError::TokenEndpoint(format!(
                "device authorization success body missing device_code; body={}",
                truncate_body(&body, 160)
            )))?;
    let user_code =
        device
            .user_code
            .filter(|s| !s.trim().is_empty())
            .ok_or(FlowError::TokenEndpoint(
                "device authorization success body missing user_code".into(),
            ))?;

    let verify_url = device
        .verification_uri_complete
        .or(device.verification_uri)
        .unwrap_or_else(|| "https://accounts.x.ai".into());

    notify.on_codes(&verify_url, &user_code);

    if open_browser && let Err(err) = open::that(verify_url.as_str()) {
        notify.on_line(&format!("(could not open browser automatically: {err})"));
    }

    let poll_every = device
        .interval
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_POLL_INTERVAL)
        .max(Duration::from_secs(1));
    let overall = device
        .expires_in
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_DEVICE_TIMEOUT);

    let tokens = timeout(
        overall,
        poll_for_tokens(&client, meta, &device_code, &pkce.verifier, poll_every),
    )
    .await
    .map_err(|_| FlowError::Timeout)??;

    save_tokens(&meta.provider_id, &tokens)?;
    notify.on_line(&format!("Login successful for {}.", meta.provider_id));
    Ok(tokens)
}

/// CLI/setup entry: eprint progress.
pub async fn run_device_login_flow(
    meta: &OauthClientMetadata,
    open_browser: bool,
) -> Result<StoredTokens, FlowError> {
    let mut notify = EprintNotify;
    run_device_login_flow_notified(meta, open_browser, &mut notify).await
}

pub fn run_device_login_flow_blocking(
    meta: &OauthClientMetadata,
    open_browser: bool,
) -> Result<StoredTokens, FlowError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| FlowError::Message(err.to_string()))?;
    rt.block_on(run_device_login_flow(meta, open_browser))
}

async fn poll_for_tokens(
    client: &reqwest::Client,
    meta: &OauthClientMetadata,
    device_code: &str,
    code_verifier: &str,
    mut interval: Duration,
) -> Result<StoredTokens, FlowError> {
    loop {
        sleep(interval).await;

        let resp = client
            .post(&meta.token_endpoint)
            .header("Accept", "application/json")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", device_code),
                ("client_id", meta.client_id.as_str()),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await?;

        let http_status = resp.status();
        let body = resp.text().await?;

        if http_status.as_u16() == 429 {
            // Back off and keep polling within overall timeout.
            interval = (interval + Duration::from_secs(10)).min(Duration::from_secs(60));
            continue;
        }

        let parsed: TokenResponse = serde_json::from_str(&body).map_err(|err| {
            FlowError::TokenEndpoint(format!(
                "invalid token poll response (HTTP {http_status}): {err}; body={}",
                truncate_body(&body, 160)
            ))
        })?;

        if let Some(err) = parsed.error.as_deref() {
            match err {
                "authorization_pending" | "slow_down" => {
                    if err == "slow_down" {
                        interval = interval.saturating_add(Duration::from_secs(5));
                    }
                    continue;
                }
                "access_denied" => {
                    return Err(FlowError::AuthorizeError(
                        parsed
                            .error_description
                            .unwrap_or_else(|| "access denied".into()),
                    ));
                }
                "expired_token" => {
                    return Err(FlowError::Timeout);
                }
                other => {
                    let detail = parsed.error_description.unwrap_or_else(|| other.to_owned());
                    if http_status.as_u16() == 403
                        || detail.to_ascii_lowercase().contains("permission")
                    {
                        return Err(FlowError::Message(format!(
                            "{detail}\n\
                             Hint: set XAI_API_KEY and auth.xai.mode = \"api_key\" if OAuth is tier-gated."
                        )));
                    }
                    return Err(FlowError::TokenEndpoint(detail));
                }
            }
        }

        let access = parsed
            .access_token
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| FlowError::TokenEndpoint("missing access_token".into()))?;

        return Ok(StoredTokens::new(
            access,
            parsed.refresh_token,
            parsed.expires_in,
            parsed.token_type,
            parsed.scope,
        ));
    }
}
