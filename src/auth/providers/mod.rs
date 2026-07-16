//! Provider-specific OAuth client metadata and adapter registry.

use {
    crate::providers::{PROVIDER_SPECS, Provider},
    serde::{Deserialize, Serialize},
    strum::VariantArray,
};

mod xai;

/// Non-secret OAuth client metadata for one provider.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OauthClientMetadata {
    pub provider_id: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    /// Device authorization endpoint (RFC 8628); required for device_code grant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_authorization_endpoint: Option<String>,
    pub client_id: String,
    pub scopes: String,
}

/// Built-in adapter for `provider`, if any.
pub fn adapter_for(provider: Provider) -> Option<OauthClientMetadata> {
    match provider {
        Provider::Xai => Some(xai::default_metadata()),
        _ => None,
    }
}

/// True when the provider has a registered OAuth adapter.
pub fn is_oauth_capable(provider: Provider) -> bool {
    adapter_for(provider).is_some()
}

/// Lowercase provider id used for token file names and CLI args.
pub fn provider_id(provider: Provider) -> String {
    provider.to_string()
}

/// All CLI provider ids (`--provider` / config / login argument).
///
/// **Source of truth:** [`PROVIDER_SPECS`] in `providers/mod.rs`.
#[allow(dead_code)] // public catalog helper for tests / future `auth list`
pub fn all_provider_ids() -> Vec<&'static str> {
    PROVIDER_SPECS
        .iter()
        .map(|spec| provider_id_static(spec.provider))
        .collect()
}

fn provider_id_static(provider: Provider) -> &'static str {
    match provider {
        Provider::Anthropic => "anthropic",
        Provider::Cerebras => "cerebras",
        Provider::Custom => "custom",
        Provider::DeepSeek => "deepseek",
        Provider::Google => "google",
        Provider::Groq => "groq",
        Provider::MiniMax => "minimax",
        Provider::Mistral => "mistral",
        Provider::OpenAi => "openai",
        Provider::OpenRouter => "openrouter",
        Provider::Xai => "xai",
        Provider::Zai => "zai",
    }
}

/// Provider ids that currently have an OAuth adapter (subset of [`all_provider_ids`]).
pub fn oauth_capable_provider_ids() -> Vec<&'static str> {
    Provider::VARIANTS
        .iter()
        .copied()
        .filter(|p| is_oauth_capable(*p))
        .map(provider_id_static)
        .collect()
}

/// Multi-line help block: every valid PROVIDER id + auth path.
///
/// Generated at runtime so `--help` cannot drift from [`PROVIDER_SPECS`].
pub fn provider_cli_help_block() -> String {
    let mut out = String::from(
        "PROVIDER — lowercase id (same as --provider / config `provider = \"…\"`)\n\
         Source of truth: greatsage/src/providers/mod.rs  PROVIDER_SPECS\n\
         OAuth adapters:   greatsage/src/auth/providers/  (is_oauth_capable)\n\
         \n\
           id            auth for `login`     API key env (non-OAuth)\n\
         ────────────  ───────────────────  ─────────────────────────\n",
    );
    for spec in PROVIDER_SPECS {
        let id = provider_id_static(spec.provider);
        let oauth = if is_oauth_capable(spec.provider) {
            "oauth (login ok)"
        } else {
            "no — use API key"
        };
        let env = spec.env_var.unwrap_or("—");
        () = out.push_str(&format!("  {id:<12}  {oauth:<19}  {env}\n"));
    }
    let oauth_ids = oauth_capable_provider_ids().join(", ");
    let oauth_example = oauth_capable_provider_ids()
        .first()
        .copied()
        .unwrap_or("xai");
    // Fixed left width so comments line up (spaces; avoids termimad `|` / tab quirks).
    const EX_W: usize = 42;
    let examples = [
        (
            format!("greatsage login {oauth_example} --device-code"),
            format!("# oauth-capable today: {oauth_ids}"),
        ),
        (format!("greatsage logout {oauth_example}"), String::new()),
        (
            "export ANTHROPIC_API_KEY=…".to_string(),
            "# key-only provider (no login)".to_string(),
        ),
        (
            "export XAI_API_KEY=…".to_string(),
            "# key path even if oauth exists".to_string(),
        ),
    ];
    () = out.push_str("\nExamples:\n");
    for (cmd, note) in examples {
        if note.is_empty() {
            () = out.push_str(&format!("  {cmd}\n"));
        } else {
            () = out.push_str(&format!("  {cmd:<EX_W$}  {note}\n"));
        }
    }
    out
}
