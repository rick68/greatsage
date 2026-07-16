//! Credential resolution: auth modes (auto / api_key / oauth).

use {
    super::{
        flow::{FlowError, refresh_access_token},
        mode::{AuthMode, auth_mode_for},
        providers::{self, OauthClientMetadata},
        store::{StoredTokens, load_tokens, save_tokens},
    },
    crate::{config::Config, providers::Provider},
};

/// Optional non-secret OAuth client metadata overrides from config.
///
/// Prefers `auth.<id>.*`, falls back to legacy `oauth.<id>.*`.
pub fn metadata_with_config_overrides(
    config: &Config,
    base: OauthClientMetadata,
) -> OauthClientMetadata {
    let id = base.provider_id.clone();
    let override_or = |key: &str, fallback: String| -> String {
        config
            .get_string(&format!("auth.{id}.{key}"))
            .ok()
            .or_else(|| config.get_string(&format!("oauth.{id}.{key}")).ok())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(fallback)
    };
    let device = config
        .get_string(&format!("auth.{id}.device_authorization_endpoint"))
        .ok()
        .or_else(|| {
            config
                .get_string(&format!("oauth.{id}.device_authorization_endpoint"))
                .ok()
        })
        .filter(|v| !v.trim().is_empty())
        .or(base.device_authorization_endpoint);
    OauthClientMetadata {
        authorization_endpoint: override_or("authorization_endpoint", base.authorization_endpoint),
        token_endpoint: override_or("token_endpoint", base.token_endpoint),
        device_authorization_endpoint: device,
        client_id: override_or("client_id", base.client_id),
        scopes: override_or("scopes", base.scopes),
        provider_id: id,
    }
}

/// Resolve adapter metadata for `provider` with optional config overrides.
pub fn resolved_metadata(
    config: Option<&Config>,
    provider: Provider,
) -> Option<OauthClientMetadata> {
    let base = providers::adapter_for(provider)?;
    Some(match config {
        Some(cfg) => metadata_with_config_overrides(cfg, base),
        None => base,
    })
}

/// Resolve the secret string for agent construct / yoagent `with_api_key`.
///
/// Modes (`auth.<provider>.mode` / `auth.default_mode`, default `auto`):
/// - `auto` — static key if set, else OAuth store
/// - `api_key` — static key only
/// - `oauth` — prefer OAuth store when usable; if no usable OAuth token (e.g. after
///   `logout`), **fall back to static API key** when configured so the session is
///   not interrupted. While OAuth tokens exist, they still win over the key.
pub fn resolve_credential(config: &Config, provider: Option<Provider>) -> Option<String> {
    let provider = provider?;
    let mode = auth_mode_for(config, provider);
    let static_key = config
        .get_api_key(Some(provider))
        .filter(|k| !k.trim().is_empty());

    match mode {
        AuthMode::ApiKey => static_key,
        AuthMode::Oauth => oauth_access(config, provider).or(static_key),
        AuthMode::Auto => static_key.or_else(|| oauth_access(config, provider)),
    }
}

fn oauth_access(config: &Config, provider: Provider) -> Option<String> {
    let provider_id = providers::provider_id(provider);
    let tokens = match load_tokens(&provider_id) {
        Ok(Some(t)) if t.has_usable_fields() => t,
        _ => return None,
    };

    if !tokens.access_token.trim().is_empty() && !tokens.access_expired() {
        return Some(tokens.access_token);
    }

    let refresh = tokens
        .refresh_token
        .as_deref()
        .filter(|t| !t.trim().is_empty())?;
    let meta = resolved_metadata(Some(config), provider)?;
    match refresh_blocking(&meta, refresh) {
        Ok(new_tokens) => {
            let access = new_tokens.access_token.clone();
            let _ = save_tokens(&provider_id, &new_tokens);
            if access.trim().is_empty() {
                None
            } else {
                Some(access)
            }
        }
        Err(_) => None,
    }
}

fn refresh_blocking(
    meta: &OauthClientMetadata,
    refresh_token: &str,
) -> Result<StoredTokens, FlowError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| FlowError::Message(err.to_string()))?;
    rt.block_on(refresh_access_token(meta, refresh_token))
}
