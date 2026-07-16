//! Auth mode: oauth vs non-oauth (api_key) vs auto.

use {
    crate::{config::Config, providers::Provider},
    std::str::FromStr,
};

/// How credentials are selected for a provider.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AuthMode {
    /// Static key if present, else OAuth store (default).
    #[default]
    Auto,
    /// Static API key only.
    ApiKey,
    /// OAuth token store only (prefer store even when a key exists).
    Oauth,
}

impl AuthMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::ApiKey => "api_key",
            Self::Oauth => "oauth",
        }
    }
}

impl FromStr for AuthMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "api_key" | "key" | "apikey" => Ok(Self::ApiKey),
            "oauth" | "oidc" => Ok(Self::Oauth),
            _ => Err(()),
        }
    }
}

/// Preferred OAuth grant for a provider.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AuthGrant {
    #[default]
    DeviceCode,
    AuthorizationCode,
}

impl AuthGrant {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeviceCode => "device_code",
            Self::AuthorizationCode => "authorization_code",
        }
    }
}

impl FromStr for AuthGrant {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "device_code" | "device" | "device-code" | "device_auth" | "device-auth" => {
                Ok(Self::DeviceCode)
            }
            "authorization_code" | "auth_code" | "loopback" | "pkce" => Ok(Self::AuthorizationCode),
            _ => Err(()),
        }
    }
}

/// Resolve effective mode for `provider` from config.
///
/// Reads `auth.<id>.mode`, then `auth.default_mode`, else [`AuthMode::Auto`].
pub fn auth_mode_for(config: &Config, provider: Provider) -> AuthMode {
    let id = provider.to_string();
    config
        .get_string(&format!("auth.{id}.mode"))
        .ok()
        .and_then(|s| AuthMode::from_str(&s).ok())
        .or_else(|| {
            config
                .get_string("auth.default_mode")
                .ok()
                .and_then(|s| AuthMode::from_str(&s).ok())
        })
        .unwrap_or_default()
}

/// Resolve grant for `provider` (default device_code for oauth-capable).
pub fn auth_grant_for(config: Option<&Config>, provider: Provider) -> AuthGrant {
    let id = provider.to_string();
    config
        .and_then(|c| c.get_string(&format!("auth.{id}.grant")).ok())
        .or_else(|| config.and_then(|c| c.get_string(&format!("oauth.{id}.grant")).ok()))
        .and_then(|s| AuthGrant::from_str(&s).ok())
        .unwrap_or(AuthGrant::DeviceCode)
}
