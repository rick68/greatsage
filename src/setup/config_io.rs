use {
    super::detect::{provider_env_var, provider_key_field},
    crate::{
        config_paths::{project_config_path, project_env_path, user_config_path, user_env_path},
        env_load::upsert_env_file,
        providers::Provider,
    },
    std::{
        env, fs, io,
        path::{Path, PathBuf},
        str::FromStr,
    },
    toml_edit::{DocumentMut, Item, Value},
};

/// Values already present in config or environment before the wizard runs.
#[derive(Clone, Debug, Default)]
pub struct ExistingSetup {
    pub provider: Option<Provider>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

pub fn load_existing_setup() -> ExistingSetup {
    let Some(path) = existing_config_path() else {
        return ExistingSetup::default();
    };
    load_existing_from_path(&path)
}

fn existing_config_path() -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    let home = dirs::home_dir()?;
    crate::config_paths::config_search_paths(&cwd, &home)
        .into_iter()
        .find(|path| crate::config_paths::config_file_is_populated(path))
}

fn load_existing_from_path(path: &Path) -> ExistingSetup {
    let Ok(content) = fs::read_to_string(path) else {
        return ExistingSetup::default();
    };
    let Ok(doc) = content.parse::<DocumentMut>() else {
        return ExistingSetup::default();
    };

    let provider = doc
        .get("provider")
        .and_then(|item| item.as_str())
        .and_then(|name| Provider::from_str(name).ok());

    let paired_env = crate::config_paths::paired_env_path(path);
    let paired_ref = paired_env.is_file().then_some(paired_env.as_path());
    let resolve =
        |raw: String| crate::env_load::resolve_credential_value_with_paired_env(&raw, paired_ref);

    let model = toml_string(doc.get("model")).and_then(resolve);
    let base_url = toml_string(doc.get("base_url")).and_then(resolve);

    let api_key = provider
        .map(provider_key_field)
        .and_then(|field| toml_string(doc.get(field)))
        .or_else(|| toml_string(doc.get("api_key")))
        .and_then(|raw| {
            crate::env_load::resolve_credential_value_with_paired_env(&raw, paired_ref)
        });

    ExistingSetup {
        provider,
        model,
        base_url,
        api_key,
    }
}

fn toml_string(item: Option<&toml_edit::Item>) -> Option<String> {
    item.and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[derive(Clone, Debug)]
pub struct WizardConfig {
    pub provider: Provider,
    pub model: String,
    pub base_url: Option<String>,
    /// Key to persist in `.env` when the user chose to save config.
    pub api_key: Option<String>,
    /// When true, key came from the environment — do not rewrite `.env`.
    pub key_from_env: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SaveResult {
    pub config: Option<PathBuf>,
    pub env: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveLocation {
    Project,
    User,
}

pub fn default_model_for_provider(provider: Provider) -> &'static str {
    provider.default_model()
}

/// Popular model ids for the setup wizard (from [`ProviderSpec::known_models`]).
pub fn known_models_for_provider(provider: Provider) -> &'static [&'static str] {
    provider.known_models()
}

pub fn wizard_project_config_path() -> io::Result<PathBuf> {
    env::current_dir().map(|cwd| project_config_path(&cwd))
}

pub fn wizard_user_config_path() -> io::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory not found"))?;
    Ok(user_config_path(&home))
}

fn credential_env_reference(provider: Provider) -> String {
    let var = provider_env_var(provider).unwrap_or("API_KEY");
    crate::env_load::format_env_reference(var)
}

pub fn save_wizard_config(path: &Path, config: &WizardConfig) -> io::Result<()> {
    let mut doc = if fs::exists(path).unwrap_or(false) {
        fs::read_to_string(path)?
            .parse::<DocumentMut>()
            .unwrap_or_default()
    } else {
        DocumentMut::new()
    };

    doc["provider"] = Item::Value(Value::from(config.provider.to_string()));
    doc["model"] = Item::Value(Value::from(config.model.as_str()));

    if let Some(base_url) = &config.base_url {
        doc["base_url"] = Item::Value(Value::from(base_url.as_str()));
    } else {
        doc.as_table_mut().remove("base_url");
    }

    let field = provider_key_field(config.provider);
    if config.api_key.is_some() {
        doc[field] = Item::Value(Value::from(credential_env_reference(config.provider)));
    } else {
        doc.as_table_mut().remove(field);
        doc.as_table_mut().remove("api_key");
    }

    fs::write(path, doc.to_string())
}

fn should_persist_env(config: &WizardConfig) -> bool {
    !config.key_from_env && config.api_key.is_some()
}

pub fn write_config(location: SaveLocation, config: &WizardConfig) -> io::Result<SaveResult> {
    let mut result = SaveResult::default();

    match location {
        SaveLocation::Project => {
            let path = wizard_project_config_path()?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            save_wizard_config(&path, config)?;
            result.config = Some(path);
            if should_persist_env(config) {
                let cwd = env::current_dir()?;
                let env_path = project_env_path(&cwd);
                let env_var = provider_env_var(config.provider).unwrap_or("API_KEY");
                upsert_env_file(
                    &env_path,
                    env_var,
                    config.api_key.as_deref().expect("checked above"),
                )?;
                result.env = Some(env_path);
            }
        }
        SaveLocation::User => {
            let path = wizard_user_config_path()?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            save_wizard_config(&path, config)?;
            result.config = Some(path);
            if should_persist_env(config) {
                let home = dirs::home_dir().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "home directory not found")
                })?;
                let env_path = user_env_path(&home);
                let env_var = provider_env_var(config.provider).unwrap_or("API_KEY");
                upsert_env_file(
                    &env_path,
                    env_var,
                    config.api_key.as_deref().expect("checked above"),
                )?;
                result.env = Some(env_path);
            }
        }
    }

    Ok(result)
}
