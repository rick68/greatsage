//! OAuth 2.1 + PKCE credential layer for public CLI clients.
//!
//! **Version label:** module docs, CLI help, and OpenSpec that say "OAuth 2.1" must stay
//! in sync with the implemented profile. Bump all of them in one change if the protocol
//! target moves (see AGENTS.md · Auth).
//!
//! Shared plumbing; static API keys remain the non-OAuth path. OAuth adapter in
//! this change: **xai only**.

pub mod device;
pub mod flow;
pub mod mode;
pub mod pkce;
pub mod providers;
pub mod resolve;
pub mod status;
pub mod store;

use {
    crate::{config::Config, providers::Provider},
    flow::{DEFAULT_LOGIN_TIMEOUT, FlowError, run_login_flow_blocking},
    mode::{AuthGrant, auth_grant_for},
    resolve::resolved_metadata,
    std::str::FromStr,
    store::delete_tokens,
};

pub use {
    providers::{is_oauth_capable, provider_cli_help_block},
    resolve::resolve_credential,
    status::status_for,
};

#[allow(unused_imports)]
pub use mode::AuthMode;

/// Options for interactive / CLI login.
#[derive(Clone, Debug)]
pub struct LoginOptions {
    pub open_browser: bool,
    /// Force device-code grant when true.
    pub device_code: bool,
    /// Force authorization-code loopback when true.
    pub authorization_code: bool,
    pub force: bool,
}

impl Default for LoginOptions {
    fn default() -> Self {
        Self {
            open_browser: true,
            device_code: false,
            authorization_code: false,
            force: false,
        }
    }
}

/// Parse optional provider CLI arg; default from config, else **xai**.
pub fn resolve_login_provider(
    config: &Config,
    provider_arg: Option<&str>,
) -> Result<Provider, String> {
    if let Some(name) = provider_arg {
        return Provider::from_str(name).map_err(|_| {
            let all = crate::providers::available_providers_line();
            format!("unknown provider `{name}`\nValid ids: {all}")
        });
    }
    Ok(config.get_provider().unwrap_or(Provider::Xai))
}

/// Run OAuth login for a provider; errors if no adapter is registered.
pub fn login_provider(
    config: Option<&Config>,
    provider: Provider,
    opts: LoginOptions,
) -> Result<(), FlowError> {
    let meta = resolved_metadata(config, provider).ok_or_else(|| {
        let oauth_ids = providers::oauth_capable_provider_ids().join(", ");
        let all = crate::providers::available_providers_line();
        FlowError::Message(format!(
            "OAuth login is not supported for provider `{provider}`.\n\
             \n\
             OAuth-capable ids (this build): {oauth_ids}\n\
             All provider ids (API keys): {all}\n\
             \n\
             Use a static API key for `{provider}`, or: greatsage login --help"
        ))
    })?;

    if !opts.force {
        let id = providers::provider_id(provider);
        if let Ok(Some(existing)) = store::load_tokens(&id)
            && existing.has_usable_fields()
            && !existing.access_expired()
        {
            eprintln!(
                "Existing OAuth tokens for `{provider}` are still present. \
                 Use --force to replace them, or: greatsage logout {provider}"
            );
            return Ok(());
        }
    }

    let grant = if opts.device_code {
        AuthGrant::DeviceCode
    } else if opts.authorization_code {
        AuthGrant::AuthorizationCode
    } else {
        auth_grant_for(config, provider)
    };

    match grant {
        AuthGrant::DeviceCode => {
            let _ = device::run_device_login_flow_blocking(&meta, opts.open_browser)?;
        }
        AuthGrant::AuthorizationCode => {
            let _ = run_login_flow_blocking(&meta, DEFAULT_LOGIN_TIMEOUT, opts.open_browser)?;
        }
    }
    Ok(())
}

/// Delete stored OAuth tokens for `provider`.
pub fn logout_provider(provider: Provider) -> Result<(), store::StoreError> {
    let id = providers::provider_id(provider);
    delete_tokens(&id)
}

/// CLI entry: login with options.
pub fn cli_login(provider_arg: Option<&str>, opts: LoginOptions) -> Result<(), i32> {
    () = env_load_for_cli();
    let config = Config::default();
    let provider = match resolve_login_provider(&config, provider_arg) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("login: {msg}");
            return Err(1);
        }
    };
    match login_provider(Some(&config), provider, opts) {
        Ok(()) => Ok(()),
        Err(err) => {
            eprintln!("login failed: {err}");
            Err(1)
        }
    }
}

/// CLI entry: `greatsage logout [provider]`.
pub fn cli_logout(provider_arg: Option<&str>) -> Result<(), i32> {
    () = env_load_for_cli();
    let config = Config::default();
    let provider = match resolve_login_provider(&config, provider_arg) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("logout: {msg}");
            return Err(1);
        }
    };
    match logout_provider(provider) {
        Ok(()) => {
            eprintln!("Logged out OAuth tokens for `{provider}`.");
            // If a static key remains, resolve falls back to it (mode=oauth or auto).
            let has_key = config
                .get_api_key(Some(provider))
                .is_some_and(|k| !k.trim().is_empty());
            if has_key {
                eprintln!(
                    "Static API key still configured for `{provider}` — will keep using it \
(no re-login required). Run `greatsage auth status {provider}` to confirm."
                );
            } else {
                eprintln!(
                    "No static API key for `{provider}`. Set an env/config key or run \
`greatsage login {provider}` before the next agent turn."
                );
            }
            Ok(())
        }
        Err(err) => {
            eprintln!("logout failed: {err}");
            Err(1)
        }
    }
}

/// CLI: print auth status (no secrets).
///
/// - **No PROVIDER:** detail for the provider that currently has an OAuth+PKCE
///   login (stored tokens). Prefer config `provider` when it has tokens; else
///   the first oauth-capable id with tokens. If none, say so (not a full catalog).
/// - **With PROVIDER:** detailed multi-line snapshot for that id only.
///
/// Full catalog table: [`cli_list`] (`auth list`).
pub fn cli_status(provider_arg: Option<&str>) -> Result<(), i32> {
    () = env_load_for_cli();
    let config = Config::default();

    let provider = if let Some(arg) = provider_arg {
        match resolve_login_provider(&config, Some(arg)) {
            Ok(p) => p,
            Err(msg) => {
                eprintln!("status: {msg}");
                return Err(1);
            }
        }
    } else {
        match provider_with_oauth_login(&config) {
            Some(p) => p,
            None => {
                println!("No OAuth + PKCE login stored.");
                println!(
                    "Use: greatsage login <provider> --device-code   (see greatsage login --help)"
                );
                println!("Catalog overview: greatsage auth list");
                return Ok(());
            }
        }
    };

    let st = status_for(&config, provider);
    for line in st.format_lines() {
        println!("{line}");
    }
    Ok(())
}

/// Provider that currently has stored OAuth tokens (login present), if any.
///
/// Prefers config `provider` when it has tokens; otherwise first catalog id with tokens.
fn provider_with_oauth_login(config: &Config) -> Option<Provider> {
    let with_tokens: Vec<Provider> = crate::providers::PROVIDER_SPECS
        .iter()
        .map(|spec| spec.provider)
        .filter(|p| status_for(config, *p).has_oauth_token)
        .collect();
    if with_tokens.is_empty() {
        return None;
    }
    if let Some(configured) = config.get_provider()
        && with_tokens.contains(&configured)
    {
        return Some(configured);
    }
    with_tokens.into_iter().next()
}

/// CLI: list catalog providers × key/token/mode.
pub fn cli_list() -> Result<(), i32> {
    () = env_load_for_cli();
    let config = Config::default();
    println!(
        "{:<12} {:<10} {:<8} {:<8} {:<10}",
        "provider", "mode", "key", "oauth", "effective"
    );
    println!("{}", "-".repeat(52));
    for spec in crate::providers::PROVIDER_SPECS {
        let st = status_for(&config, spec.provider);
        println!(
            "{:<12} {:<10} {:<8} {:<8} {:<10}",
            st.provider,
            st.mode.as_str(),
            if st.has_static_key { "yes" } else { "no" },
            if st.has_oauth_token { "yes" } else { "no" },
            st.effective
        );
    }
    Ok(())
}

fn env_load_for_cli() {
    crate::env_load::load_layered_env();
}
