//! Non-secret auth status for CLI / TUI.

use {
    super::{
        mode::{AuthMode, auth_mode_for},
        providers,
        store::load_tokens,
    },
    crate::{config::Config, providers::Provider},
};

/// Snapshot of how a provider will authenticate (no secrets).
#[derive(Clone, Debug)]
pub struct AuthStatus {
    pub provider: String,
    pub mode: AuthMode,
    pub has_static_key: bool,
    pub has_oauth_token: bool,
    pub oauth_expired: bool,
    pub oauth_capable: bool,
    /// What resolve would use right now: `api_key` | `oauth` | `none`.
    pub effective: &'static str,
}

impl AuthStatus {
    /// One-line chrome for TUI / status, e.g. `auth:xai·oauth`.
    pub fn chrome_label(&self) -> String {
        format!("auth:{}·{}", self.provider, self.effective)
    }

    pub fn format_lines(&self) -> Vec<String> {
        vec![
            format!("provider:        {}", self.provider),
            format!("config mode:     {}", self.mode.as_str()),
            format!("oauth-capable:   {}", self.oauth_capable),
            format!("has static key:  {}", self.has_static_key),
            format!(
                "has oauth token: {}{}",
                self.has_oauth_token,
                if self.oauth_expired {
                    " (access expired or needs refresh)"
                } else {
                    ""
                }
            ),
            format!("effective:       {}", self.effective),
        ]
    }
}

pub fn status_for(config: &Config, provider: Provider) -> AuthStatus {
    let mode = auth_mode_for(config, provider);
    let has_static_key = config
        .get_api_key(Some(provider))
        .is_some_and(|k| !k.trim().is_empty());
    let id = providers::provider_id(provider);
    let (has_oauth_token, oauth_expired) = match load_tokens(&id) {
        Ok(Some(t)) if t.has_usable_fields() => {
            let expired = t.access_expired() || t.access_token.trim().is_empty();
            (true, expired && t.refresh_token.is_none())
        }
        Ok(Some(_)) => (false, false),
        _ => (false, false),
    };
    let oauth_capable = providers::is_oauth_capable(provider);

    let effective = match mode {
        AuthMode::ApiKey => {
            if has_static_key {
                "api_key"
            } else {
                "none"
            }
        }
        AuthMode::Oauth => {
            if has_oauth_token && !oauth_expired {
                "oauth"
            } else if has_oauth_token {
                "oauth" // refresh may still work
            } else {
                "none"
            }
        }
        AuthMode::Auto => {
            if has_static_key {
                "api_key"
            } else if has_oauth_token {
                "oauth"
            } else {
                "none"
            }
        }
    };

    AuthStatus {
        provider: id,
        mode,
        has_static_key,
        has_oauth_token,
        oauth_expired,
        oauth_capable,
        effective,
    }
}
