//! OAuth 2.0 Authorization Code + PKCE loopback flow and token endpoint calls.

use {
    super::{
        pkce::{PkceSession, generate_pkce_session},
        providers::OauthClientMetadata,
        store::{StoredTokens, save_tokens},
    },
    serde::Deserialize,
    std::{collections::HashMap, io, time::Duration},
    thiserror::Error,
    tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::timeout,
    },
    url::Url,
};

/// Default how long to wait for the browser callback.
pub const DEFAULT_LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Error)]
pub enum FlowError {
    #[error("oauth login timed out (device approval or browser callback)")]
    Timeout,
    #[error("oauth state mismatch (possible CSRF); login aborted")]
    StateMismatch,
    #[error("authorization server returned error: {0}")]
    AuthorizeError(String),
    #[error("missing authorization code in callback")]
    MissingCode,
    #[error("token endpoint error: {0}")]
    TokenEndpoint(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("http client error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("url error: {0}")]
    Url(#[from] url::ParseError),
    #[error("token store error: {0}")]
    Store(#[from] super::store::StoreError),
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
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

/// Build the authorization URL (does not start a listener).
pub fn build_authorize_url(
    meta: &OauthClientMetadata,
    pkce: &PkceSession,
    redirect_uri: &str,
) -> Result<Url, FlowError> {
    let mut url = Url::parse(&meta.authorization_endpoint)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("response_type", "code");
        pairs.append_pair("client_id", &meta.client_id);
        pairs.append_pair("redirect_uri", redirect_uri);
        pairs.append_pair("scope", &meta.scopes);
        pairs.append_pair("state", &pkce.state);
        pairs.append_pair("code_challenge", &pkce.challenge);
        pairs.append_pair("code_challenge_method", "S256");
    }
    Ok(url)
}

/// Exchange authorization code for tokens (no secrets logged).
pub async fn exchange_code(
    meta: &OauthClientMetadata,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<StoredTokens, FlowError> {
    let client = reqwest::Client::new();
    let resp = client
        .post(&meta.token_endpoint)
        .header("Accept", "application/json")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", &meta.client_id),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await?;

    parse_token_response(resp).await
}

/// Refresh access token using a refresh_token grant.
pub async fn refresh_access_token(
    meta: &OauthClientMetadata,
    refresh_token: &str,
) -> Result<StoredTokens, FlowError> {
    let client = reqwest::Client::new();
    let resp = client
        .post(&meta.token_endpoint)
        .header("Accept", "application/json")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &meta.client_id),
        ])
        .send()
        .await?;

    let mut tokens = parse_token_response(resp).await?;
    // Some ASs omit refresh_token on refresh; keep the old one.
    if tokens.refresh_token.is_none() {
        tokens.refresh_token = Some(refresh_token.to_owned());
    }
    Ok(tokens)
}

async fn parse_token_response(resp: reqwest::Response) -> Result<StoredTokens, FlowError> {
    let status = resp.status();
    let body = resp.text().await?;
    let parsed: TokenResponse = serde_json::from_str(&body).map_err(|err| {
        FlowError::TokenEndpoint(format!("invalid token response (HTTP {status}): {err}"))
    })?;

    if let Some(err) = parsed.error {
        let detail = parsed.error_description.unwrap_or_else(|| err.clone());
        return Err(FlowError::TokenEndpoint(detail));
    }

    if parsed.access_token.trim().is_empty() {
        return Err(FlowError::TokenEndpoint(
            "token response missing access_token".into(),
        ));
    }

    if !status.is_success() {
        return Err(FlowError::TokenEndpoint(format!(
            "HTTP {status} from token endpoint"
        )));
    }

    Ok(StoredTokens::new(
        parsed.access_token,
        parsed.refresh_token,
        parsed.expires_in,
        parsed.token_type,
        parsed.scope,
    ))
}

/// Full interactive login: loopback + browser + exchange + save.
///
/// Operator-facing messages never include verifiers or tokens.
pub async fn run_login_flow(
    meta: &OauthClientMetadata,
    login_timeout: Duration,
    open_browser: bool,
) -> Result<StoredTokens, FlowError> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let pkce = generate_pkce_session();
    let authorize_url = build_authorize_url(meta, &pkce, &redirect_uri)?;

    eprintln!("Opening browser for {} login…", meta.provider_id);
    eprintln!("If the browser does not open, visit:\n  {authorize_url}");

    if open_browser && let Err(err) = open::that(authorize_url.as_str()) {
        // Still allow manual visit; do not fail solely on open.
        eprintln!("(could not open browser automatically: {err})");
    }

    let (code, state) = timeout(login_timeout, accept_callback(&listener))
        .await
        .map_err(|_| FlowError::Timeout)??;

    if state != pkce.state {
        return Err(FlowError::StateMismatch);
    }

    let tokens = exchange_code(meta, &code, &pkce.verifier, &redirect_uri).await?;
    save_tokens(&meta.provider_id, &tokens)?;
    eprintln!("Login successful for {}.", meta.provider_id);
    Ok(tokens)
}

/// Blocking wrapper for CLI / setup (owns a short-lived Tokio runtime).
pub fn run_login_flow_blocking(
    meta: &OauthClientMetadata,
    login_timeout: Duration,
    open_browser: bool,
) -> Result<StoredTokens, FlowError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| FlowError::Message(err.to_string()))?;
    rt.block_on(run_login_flow(meta, login_timeout, open_browser))
}

async fn accept_callback(listener: &TcpListener) -> Result<(String, String), FlowError> {
    let (mut socket, _) = listener.accept().await?;
    let mut buf = vec![0u8; 8192];
    let n = socket.read(&mut buf).await?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let path_line = req.lines().next().unwrap_or("");
    // GET /callback?code=...&state=... HTTP/1.1
    let path = path_line.split_whitespace().nth(1).unwrap_or("/");

    let full = format!("http://127.0.0.1{path}");
    let url = Url::parse(&full)?;
    let pairs: HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    let body = if let Some(err) = pairs.get("error") {
        let desc = pairs
            .get("error_description")
            .cloned()
            .unwrap_or_else(|| err.clone());
        let html = html_page("Login failed", &format!("Authorization error: {desc}"));
        let _ = write_http_response(&mut socket, 400, &html).await;
        return Err(FlowError::AuthorizeError(desc));
    } else {
        html_page(
            "Login complete",
            "You can close this window and return to greatsage.",
        )
    };

    let code = pairs
        .get("code")
        .cloned()
        .filter(|c| !c.is_empty())
        .ok_or(FlowError::MissingCode);
    let state = pairs.get("state").cloned().unwrap_or_default();

    let status = if code.is_ok() { 200 } else { 400 };
    let _ = write_http_response(&mut socket, status, &body).await;

    let code = code?;
    if state.is_empty() {
        return Err(FlowError::StateMismatch);
    }
    Ok((code, state))
}

async fn write_http_response(
    socket: &mut tokio::net::TcpStream,
    status: u16,
    body: &str,
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await?;
    socket.shutdown().await?;
    Ok(())
}

fn html_page(title: &str, message: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{title}</title></head>\
         <body style=\"font-family: system-ui, sans-serif; padding: 2rem;\">\
         <h1>{title}</h1><p>{message}</p></body></html>"
    )
}
