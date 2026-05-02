use clap::ValueEnum;

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum, serde::Deserialize)]
pub enum Provider {
    Anthropic,
    Bedrock,
    Cerebras,
    Custom,
    Deepseek,
    Google,
    Groq,
    Minimax,
    Mistral,
    Ollama,
    Openai,
    Openrouter,
    Xai,
    Zai,
}
