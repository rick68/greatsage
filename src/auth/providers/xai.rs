//! xAI OAuth adapter (OAuth 2.1 + PKCE; device code default).
//!
//! Defaults align with public CLI practice (e.g. Hermes `xai-oauth` / Grok Build
//! `auth.x.ai`). Override via `[auth.xai]` or legacy `oauth.xai.*` in config.

use super::OauthClientMetadata;

/// Authorization endpoint (authorization_code grant).
pub const DEFAULT_AUTHORIZATION_ENDPOINT: &str = "https://auth.x.ai/oauth2/authorize";
/// Token endpoint.
pub const DEFAULT_TOKEN_ENDPOINT: &str = "https://auth.x.ai/oauth2/token";
/// Device authorization endpoint (device_code grant — product default).
pub const DEFAULT_DEVICE_AUTHORIZATION_ENDPOINT: &str = "https://auth.x.ai/oauth2/device/code";
/// Public client id used by common xAI CLI OAuth flows (overridable in config).
pub const DEFAULT_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
/// Scopes including offline_access for refresh + API access.
pub const DEFAULT_SCOPES: &str = "openid profile email offline_access grok-cli:access api:access";

pub const PROVIDER_ID: &str = "xai";

pub fn default_metadata() -> OauthClientMetadata {
    OauthClientMetadata {
        provider_id: PROVIDER_ID.to_owned(),
        authorization_endpoint: DEFAULT_AUTHORIZATION_ENDPOINT.to_owned(),
        token_endpoint: DEFAULT_TOKEN_ENDPOINT.to_owned(),
        device_authorization_endpoint: Some(DEFAULT_DEVICE_AUTHORIZATION_ENDPOINT.to_owned()),
        client_id: DEFAULT_CLIENT_ID.to_owned(),
        scopes: DEFAULT_SCOPES.to_owned(),
    }
}
