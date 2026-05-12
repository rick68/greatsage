use {
    crate::{Cli, agents::SYSTEM_PROMPT},
    bevy::{
        app::{App, PreStartup},
        ecs::{change_detection::Res, resource::Resource, system::Commands},
        prelude::Deref,
    },
    config::builder::DefaultState,
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::{env, fs, path::PathBuf},
    toml_edit::{DocumentMut, Value},
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
        if let Ok(cwd) = env::current_dir()
            && let Some(project_level_config) = Some(cwd.join(".greatsage.toml"))
            && fs::exists(&project_level_config).unwrap_or_default()
        {
            return project_level_config.canonicalize().unwrap();
        }

        let home = dirs::home_dir().unwrap();

        if let Some(home_directory_config) = Some(home.join(".greatsage.toml"))
            && fs::exists(&home_directory_config).unwrap_or_default()
        {
            return home_directory_config.canonicalize().unwrap();
        }

        let user_level_conifg = home.join(".config").join("greatsage").join("config.toml");
        if !fs::exists(&user_level_conifg).unwrap_or_default()
            && let Some(parent) = user_level_conifg.parent()
        {
            let _ = fs::create_dir_all(parent);
            let _ = fs::File::create(&user_level_conifg);
        }

        user_level_conifg.canonicalize().unwrap()
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
        } else if let Ok(anthropic_api_key) = dotenvy::var("ANTHROPIC_API_KEY") {
            config_builder = config_builder
                .clone()
                .set_override("api_key", anthropic_api_key)
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
    pub fn get_model(&self) -> Option<String> {
        self.get_string("model").ok()
    }

    #[allow(dead_code)]
    pub fn set_model(&mut self, model: impl Into<Value>) {
        if let Ok(content) = fs::read_to_string(Self::config_file())
            && let Ok(mut doc) = content.parse::<DocumentMut>()
        {
            doc["model"] = toml_edit::Item::Value(model.into());
            let _ = fs::write(Self::config_file(), doc.to_string());
        }
    }

    pub fn get_base_url(&self) -> Option<String> {
        self.get_string("base_url").ok()
    }

    #[allow(dead_code)]
    pub fn set_base_url(&mut self, base_url: impl Into<Value>) {
        if let Ok(content) = fs::read_to_string(Self::config_file())
            && let Ok(mut doc) = content.parse::<DocumentMut>()
        {
            doc["base_url"] = toml_edit::Item::Value(base_url.into());
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

    pub fn get_api_key(&self) -> Option<String> {
        self.get_string("api_key").ok()
    }

    #[allow(dead_code)]
    pub fn set_api_key(&mut self, api_key: impl Into<Value>) {
        if let Ok(content) = fs::read_to_string(Self::config_file())
            && let Ok(mut doc) = content.parse::<DocumentMut>()
        {
            doc["api_key"] = toml_edit::Item::Value(api_key.into());
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
}

fn setup(mut commands: Commands, cli: Res<Cli>) {
    commands.insert_resource(Config::from(cli.into_inner()));
}

pub(crate) fn config_plugin(app: &mut App) {
    app.add_systems(PreStartup, setup);
}
