mod provider_models;

use {
    clap::ValueEnum,
    provider_models::{
        ANTHROPIC_KNOWN_MODELS, CEREBRAS_KNOWN_MODELS, DEEPSEEK_KNOWN_MODELS, GOOGLE_KNOWN_MODELS,
        GROQ_KNOWN_MODELS, MINIMAX_KNOWN_MODELS, MISTRAL_KNOWN_MODELS, OPENAI_KNOWN_MODELS,
        OPENROUTER_KNOWN_MODELS, XAI_KNOWN_MODELS, ZAI_KNOWN_MODELS,
    },
    strum::{Display, EnumString, VariantArray},
};

#[derive(
    Clone, Copy, Debug, Default, Display, EnumString, Eq, PartialEq, ValueEnum, VariantArray,
)]
#[strum(serialize_all = "lowercase")]
pub enum Provider {
    Anthropic,
    Cerebras,
    #[default]
    Custom,
    #[value(name = "deepseek")]
    DeepSeek,
    Google,
    Groq,
    #[value(name = "minimax")]
    MiniMax,
    Mistral,
    #[value(name = "openai")]
    OpenAi,
    #[value(name = "openrouter")]
    OpenRouter,
    Xai,
    Zai,
}

/// Setup wizard menu order — one entry per [`Provider`] variant.
pub const PROVIDER_SPECS: &[ProviderSpec] = &[
    ProviderSpec {
        provider: Provider::Anthropic,
        wizard_label: "Anthropic",
        env_var: Some("ANTHROPIC_API_KEY"),
        config_key: "anthropic_api_key",
        default_model: "claude-fable-5",
        known_models: ANTHROPIC_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::Cerebras,
        wizard_label: "Cerebras",
        env_var: Some("CEREBRAS_API_KEY"),
        config_key: "cerebras_api_key",
        default_model: "gpt-oss-120b",
        known_models: CEREBRAS_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::Custom,
        wizard_label: "Custom / local (OpenAI-compatible)",
        env_var: Some("API_KEY"),
        config_key: "api_key",
        default_model: "gpt-5.5",
        known_models: &[],
    },
    ProviderSpec {
        provider: Provider::DeepSeek,
        wizard_label: "DeepSeek",
        env_var: Some("DEEPSEEK_API_KEY"),
        config_key: "deepseek_api_key",
        default_model: "deepseek-v4-pro",
        known_models: DEEPSEEK_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::Google,
        wizard_label: "Google (Gemini)",
        env_var: Some("GOOGLE_API_KEY"),
        config_key: "google_api_key",
        default_model: "gemini-3.1-pro-preview",
        known_models: GOOGLE_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::Groq,
        wizard_label: "Groq",
        env_var: Some("GROQ_API_KEY"),
        config_key: "groq_api_key",
        default_model: "openai/gpt-oss-120b",
        known_models: GROQ_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::MiniMax,
        wizard_label: "MiniMax",
        env_var: Some("MINIMAX_API_KEY"),
        config_key: "minimax_api_key",
        default_model: "MiniMax-M3",
        known_models: MINIMAX_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::Mistral,
        wizard_label: "Mistral",
        env_var: Some("MISTRAL_API_KEY"),
        config_key: "mistral_api_key",
        default_model: "mistral-large-2512",
        known_models: MISTRAL_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::OpenAi,
        wizard_label: "OpenAI",
        env_var: Some("OPENAI_API_KEY"),
        config_key: "openai_api_key",
        default_model: "gpt-5.5",
        known_models: OPENAI_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::OpenRouter,
        wizard_label: "OpenRouter",
        env_var: Some("OPENROUTER_API_KEY"),
        config_key: "openrouter_api_key",
        default_model: "anthropic/claude-fable-5",
        known_models: OPENROUTER_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::Xai,
        wizard_label: "xAI",
        env_var: Some("XAI_API_KEY"),
        config_key: "xai_api_key",
        default_model: "grok-4.5",
        known_models: XAI_KNOWN_MODELS,
    },
    ProviderSpec {
        provider: Provider::Zai,
        wizard_label: "ZAI (Zhipu AI / z.ai)",
        env_var: Some("ZAI_API_KEY"),
        config_key: "zai_api_key",
        default_model: "glm-5.2",
        known_models: ZAI_KNOWN_MODELS,
    },
];

pub fn provider_spec(provider: Provider) -> &'static ProviderSpec {
    PROVIDER_SPECS
        .iter()
        .find(|spec| spec.provider == provider)
        .unwrap_or_else(|| panic!("missing ProviderSpec for {provider}"))
}

/// Lowercase provider ids from [`PROVIDER_SPECS`], sorted with `custom` last (yoyo-style `/provider` show).
pub fn available_provider_names() -> Vec<String> {
    let mut names: Vec<String> = PROVIDER_SPECS
        .iter()
        .map(|spec| spec.provider.to_string())
        .collect();
    names.sort();
    if let Some(pos) = names.iter().position(|name| name == "custom") {
        let custom = names.remove(pos);
        names.push(custom);
    }
    names
}

pub fn available_providers_line() -> String {
    available_provider_names().join(", ")
}

/// Per-provider metadata: wizard label, env var, config key, and default models.
#[derive(Clone, Copy, Debug)]
pub struct ProviderSpec {
    pub provider: Provider,
    pub wizard_label: &'static str,
    pub env_var: Option<&'static str>,
    pub config_key: &'static str,
    pub default_model: &'static str,
    pub known_models: &'static [&'static str],
}

impl Provider {
    pub fn spec(self) -> &'static ProviderSpec {
        provider_spec(self)
    }

    pub fn env_var(self) -> Option<&'static str> {
        self.spec().env_var
    }

    pub fn config_key_field(self) -> &'static str {
        self.spec().config_key
    }

    pub fn default_model(self) -> &'static str {
        self.spec().default_model
    }

    pub fn known_models(self) -> &'static [&'static str] {
        self.spec().known_models
    }

    pub fn wizard_label(self) -> &'static str {
        self.spec().wizard_label
    }
}

impl From<&Provider> for config::ValueKind {
    fn from(provider: &Provider) -> Self {
        config::ValueKind::String(provider.to_string())
    }
}
