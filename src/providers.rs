use {
    clap::ValueEnum,
    strum::{Display, EnumString},
};

#[derive(Clone, Copy, Debug, Default, Display, EnumString, Eq, PartialEq, ValueEnum)]
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

impl From<&Provider> for config::ValueKind {
    fn from(provider: &Provider) -> Self {
        config::ValueKind::String(provider.to_string())
    }
}

impl Into<String> for Provider {
    fn into(self) -> String {
        self.to_string()
    }
}
