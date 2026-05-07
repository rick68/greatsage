use {
    crate::Cli,
    bevy::{
        app::{App, PreStartup},
        ecs::{change_detection::Res, resource::Resource, system::Commands},
        prelude::Deref,
    },
    config::builder::DefaultState,
    std::{fs, path::PathBuf},
    toml_edit::{DocumentMut, Value},
};

#[derive(Deref, Resource)]
pub struct Config(config::Config);

impl Config {
    #[inline]
    fn get_config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".config")
            .join("greatsage")
    }

    #[inline]
    fn get_config_file() -> PathBuf {
        Self::get_config_dir().join("config.toml")
    }

    fn get_builder() -> config::ConfigBuilder<DefaultState> {
        let config_dir = Self::get_config_dir();
        let config_file = Self::get_config_file();

        if let Ok(false) = fs::exists(&config_dir) {
            let _ = fs::create_dir_all(&config_dir);
        }

        if let Ok(false) = fs::exists(&config_file) {
            let _ = fs::File::create(&config_file);
        }

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
                .set_override("base_url", base_url.clone())
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

        Config(config_builder.build().unwrap_or_default())
    }
}

impl Config {
    pub fn get_model(&self) -> Option<String> {
        self.get_string("model").ok()
    }

    #[allow(dead_code)]
    pub fn set_model(&mut self, model: impl Into<Value>) {
        if let Ok(content) = fs::read_to_string(Self::get_config_file())
            && let Ok(mut doc) = content.parse::<DocumentMut>()
        {
            doc["model"] = toml_edit::Item::Value(model.into());
            let _ = fs::write(Self::get_config_file(), doc.to_string());
        }
    }

    pub fn get_base_url(&self) -> Option<String> {
        self.get_string("base_url").ok()
    }

    #[allow(dead_code)]
    pub fn set_base_url(&mut self, base_url: impl Into<Value>) {
        if let Ok(content) = fs::read_to_string(Self::get_config_file())
            && let Ok(mut doc) = content.parse::<DocumentMut>()
        {
            doc["base_url"] = toml_edit::Item::Value(base_url.into());
            let _ = fs::write(Self::get_config_file(), doc.to_string());
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

    pub fn get_api_key(&self) -> Option<String> {
        self.get_string("api_key").ok()
    }

    #[allow(dead_code)]
    pub fn set_api_key(&mut self, api_key: impl Into<Value>) {
        if let Ok(content) = fs::read_to_string(Self::get_config_file())
            && let Ok(mut doc) = content.parse::<DocumentMut>()
        {
            doc["api_key"] = toml_edit::Item::Value(api_key.into());
            let _ = fs::write(Self::get_config_file(), doc.to_string());
        }
    }
}

fn setup(mut commands: Commands, cli: Res<Cli>) {
    commands.insert_resource(Config::from(cli.into_inner()))
}

pub(crate) fn config_plugin(app: &mut App) {
    app.add_systems(PreStartup, setup);
}
