use {
    crate::{
        Cli, agents::SYSTEM_PROMPT, env_load::resolve_credential_value_with_paired_env,
        providers::Provider,
    },
    bevy::{
        app::{App, PreStartup},
        ecs::{change_detection::Res, resource::Resource, system::Commands},
        prelude::Deref,
    },
    config::builder::DefaultState,
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::{env, fs, path::PathBuf, str::FromStr},
    toml_edit::{DocumentMut, Item, Value},
    url::Url,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum McpConfig {
    SseTransports(Url),
    StdioTransports(String),
}

impl From<String> for McpConfig {
    fn from(value: String) -> Self {
        if let Ok(url) = Url::parse(&value) {
            McpConfig::SseTransports(url)
        } else {
            McpConfig::StdioTransports(value)
        }
    }
}

#[derive(Clone, Debug, Deref, Resource)]
pub struct Config(config::Config);

impl Config {
    #[inline]
    fn config_file() -> PathBuf {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let home = dirs::home_dir().unwrap();

        for path in crate::config_paths::config_search_paths(&cwd, &home) {
            if crate::config_paths::config_file_is_populated(&path) {
                return path.canonicalize().unwrap_or(path);
            }
        }

        let default = crate::config_paths::user_config_path(&home);
        if let Some(parent) = default.parent() {
            let _ = fs::create_dir_all(parent);
            let _ = fs::File::create(&default);
        }

        default.canonicalize().unwrap_or(default)
    }

    fn get_builder() -> config::ConfigBuilder<DefaultState> {
        let config_file = Self::config_file();
        config::Config::builder().add_source(
            config::File::with_name(format!("{}", config_file.display()).as_str()).required(true),
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Config(Config::get_builder().build().unwrap_or_default())
    }
}

impl From<&Cli> for Config {
    fn from(cli: &Cli) -> Self {
        let mut config_builder = Self::get_builder();
        let mut current_provider = None;

        if let Some(model) = &cli.model {
            config_builder = config_builder
                .clone()
                .set_override("model", model.clone())
                .unwrap_or(config_builder);
        }

        if let Some(base_url) = &cli.base_url {
            config_builder = config_builder
                .clone()
                .set_override("base_url", base_url.as_str())
                .unwrap_or(config_builder);
        };

        if let Some(skills) = &cli.skills {
            config_builder = config_builder
                .clone()
                .set_override(
                    "skills",
                    skills
                        .iter()
                        .map(|path| format!("{}", path.display()))
                        .collect::<Vec<String>>(),
                )
                .unwrap_or(config_builder);
        };

        match (&cli.system, &cli.system_file) {
            (Some(system_prompt), None) => {
                config_builder = config_builder
                    .clone()
                    .set_override("system_prompt", system_prompt.clone())
                    .unwrap_or(config_builder);
            }
            (system_prompt, Some(system_prompt_file)) => {
                let system_prompt = {
                    if fs::exists(system_prompt_file).unwrap_or_default()
                        && let Ok(system_prompt) = fs::read_to_string(system_prompt_file)
                    {
                        system_prompt
                    } else {
                        system_prompt.clone().unwrap_or(String::from(SYSTEM_PROMPT))
                    }
                };

                config_builder = config_builder
                    .clone()
                    .set_override("system_prompt", system_prompt)
                    .unwrap_or(config_builder);
            }
            _ => (),
        }

        if let Some(api_key) = &cli.api_key {
            config_builder = config_builder
                .clone()
                .set_override("api_key", api_key.clone())
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::Custom);
        }
        if let Ok(api_key) = dotenvy::var("API_KEY")
            && !api_key.trim().is_empty()
        {
            config_builder = config_builder
                .clone()
                .set_override("api_key", api_key)
                .unwrap_or(config_builder);
        }
        if let Ok(anthropic_api_key) = dotenvy::var("ANTHROPIC_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("anthropic_api_key", anthropic_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::Anthropic);
        }
        if let Ok(cerebras_api_key) = dotenvy::var("CEREBRAS_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("cerebras_api_key", cerebras_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::Cerebras);
        }
        if let Ok(deepseek_api_key) = dotenvy::var("DEEPSEEK_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("deepseek_api_key", deepseek_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::DeepSeek);
        }
        if let Ok(google_api_key) = dotenvy::var("GOOGLE_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("google_api_key", google_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::Google);
        }
        if let Ok(groq_api_key) = dotenvy::var("GROQ_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("groq_api_key", groq_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::Groq);
        }
        if let Ok(minimax_api_key) = dotenvy::var("MINIMAX_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("minimax_api_key", minimax_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::MiniMax);
        }
        if let Ok(mistral_api_key) = dotenvy::var("MISTRAL_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("mistral_api_key", mistral_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::Mistral);
        }
        if let Ok(openai_api_key) = dotenvy::var("OPENAI_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("openai_api_key", openai_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::OpenAi);
        }
        if let Ok(openrouter_api_key) = dotenvy::var("OPENROUTER_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("openrouter_api_key", openrouter_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::OpenRouter);
        }
        if let Ok(xai_api_key) = dotenvy::var("XAI_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("xai_api_key", xai_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::Xai);
        }
        if let Ok(zai_api_key) = dotenvy::var("ZAI_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("zai_api_key", zai_api_key)
                .unwrap_or(config_builder);
            current_provider = Some(&Provider::Zai);
        }

        if let Some(provider) = cli.provider.as_ref().or(current_provider) {
            config_builder = config_builder
                .clone()
                .set_override("provider", provider)
                .unwrap_or(config_builder);
        }

        if let Some(mcp) = &cli.mcp {
            config_builder = config_builder
                .clone()
                .set_override("mcp", json!(mcp).to_string())
                .unwrap_or(config_builder);
        };

        Config(config_builder.build().unwrap_or_default())
    }
}

impl Config {
    fn resolve_config_field(&self, key: &str) -> Option<String> {
        let raw = self.get_string(key).ok()?;
        let paired_env = crate::config_paths::paired_env_path(&Self::config_file());
        let paired_ref = paired_env.is_file().then_some(paired_env.as_path());
        resolve_credential_value_with_paired_env(&raw, paired_ref)
    }

    pub fn get_model(&self) -> Option<String> {
        self.resolve_config_field("model")
    }

    #[allow(dead_code)]
    pub fn set_model(&mut self, model: impl Into<Value>) {
        if let Ok(content) = fs::read_to_string(Self::config_file())
            && let Ok(mut doc) = content.parse::<DocumentMut>()
        {
            doc["model"] = Item::Value(model.into());
            let _ = fs::write(Self::config_file(), doc.to_string());
        }
    }

    pub fn get_provider(&self) -> Option<Provider> {
        Provider::from_str(self.get_string("provider").ok()?.as_str()).ok()
    }

    pub fn get_base_url(&self) -> Option<String> {
        self.resolve_config_field("base_url")
    }

    #[allow(dead_code)]
    pub fn set_base_url(&mut self, base_url: impl Into<Value>) {
        if let Ok(content) = fs::read_to_string(Self::config_file())
            && let Ok(mut doc) = content.parse::<DocumentMut>()
        {
            doc["base_url"] = Item::Value(base_url.into());
            let _ = fs::write(Self::config_file(), doc.to_string());
        }
    }

    pub fn get_skills(&self) -> Vec<PathBuf> {
        self.get_array("skills")
            .map(|skill| {
                skill
                    .into_iter()
                    .map(|v| v.into_string().unwrap_or_default())
                    .map(PathBuf::from)
                    .filter(|path| fs::exists(path).unwrap_or(false))
                    .collect::<Vec<PathBuf>>()
            })
            .unwrap_or_default()
    }

    pub fn get_system_prompt(&self) -> String {
        self.get_string("system_prompt")
            .unwrap_or(String::from(SYSTEM_PROMPT))
    }

    pub fn get_api_key(&self, provider: Option<Provider>) -> Option<String> {
        let paired_env = crate::config_paths::paired_env_path(&Self::config_file());
        let paired_ref = paired_env.is_file().then_some(paired_env.as_path());
        let resolve = |value: String| {
            crate::env_load::resolve_credential_value_with_paired_env(&value, paired_ref)
        };

        let api_key = self.get_string("api_key").ok().and_then(resolve);
        match provider {
            Some(Provider::Anthropic) => self.get_string("anthropic_api_key").ok(),
            Some(Provider::Cerebras) => self.get_string("cerebras_api_key").ok(),
            Some(Provider::DeepSeek) => self.get_string("deepseek_api_key").ok(),
            Some(Provider::Google) => self.get_string("google_api_key").ok(),
            Some(Provider::Groq) => self.get_string("groq_api_key").ok(),
            Some(Provider::MiniMax) => self.get_string("minimax_api_key").ok(),
            Some(Provider::Mistral) => self.get_string("mistral_api_key").ok(),
            Some(Provider::OpenAi) => self.get_string("openai_api_key").ok(),
            Some(Provider::OpenRouter) => self.get_string("openrouter_api_key").ok(),
            Some(Provider::Xai) => self.get_string("xai_api_key").ok(),
            Some(Provider::Zai) => self.get_string("zai_api_key").ok(),
            Some(Provider::Custom) | None => api_key.clone(),
        }
        .or(api_key)
        .and_then(resolve)
    }

    #[allow(dead_code)]
    pub fn set_api_key(&mut self, api_key: impl Into<Value>) {
        if let Ok(content) = fs::read_to_string(Self::config_file())
            && let Ok(mut doc) = content.parse::<DocumentMut>()
        {
            doc["api_key"] = Item::Value(api_key.into());
            let _ = fs::write(Self::config_file(), doc.to_string());
        }
    }

    pub fn get_mcp(&self) -> Vec<McpConfig> {
        if let Ok(array) = self.get_array("mcp") {
            array
                .into_iter()
                .map(|v| v.into_string().unwrap_or_default())
                .filter(|s| !s.is_empty())
                .map(McpConfig::from)
                .collect::<Vec<McpConfig>>()
        } else {
            vec![]
        }
    }

    /// `[session]` retention keys in config.toml:
    /// - `max_turns` — max `TurnSummary` entities per session before prune (default 200)
    /// - `max_tool_records` — max `ToolCallRecord` entities per session before prune (default 2000)
    pub fn session_limits(&self) -> (usize, usize) {
        (
            self.get_int("session.max_turns").unwrap_or(200) as usize,
            self.get_int("session.max_tool_records").unwrap_or(2000) as usize,
        )
    }
}

fn setup(mut commands: Commands, cli: Res<Cli>) {
    commands.insert_resource(Config::from(cli.into_inner()));
}

pub(crate) fn config_plugin(app: &mut App) {
    app.add_systems(PreStartup, setup);
}
