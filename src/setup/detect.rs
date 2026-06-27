use {
    crate::{
        config_paths::{CONFIG_FILENAME, config_search_paths, paired_env_path},
        env_load::credential_value_is_set_with_paired_env,
        providers::{PROVIDER_SPECS, Provider},
    },
    std::{
        env, fs,
        path::{Path, PathBuf},
        str::FromStr,
    },
    toml_edit::DocumentMut,
};

/// Returns true when no usable API credentials exist (env or TOML content).
pub fn needs_setup() -> bool {
    !has_credentials()
}

pub(super) fn provider_env_var(provider: Provider) -> Option<&'static str> {
    provider.env_var()
}

pub(super) fn provider_key_field(provider: Provider) -> &'static str {
    provider.config_key_field()
}

fn credential_field_is_set(item: Option<&toml_edit::Item>, paired_env: Option<&Path>) -> bool {
    item.and_then(|item| item.as_str())
        .is_some_and(|value| credential_value_is_set_with_paired_env(value, paired_env))
}

fn item_as_non_empty_str(item: Option<&toml_edit::Item>) -> bool {
    item.and_then(|item| item.as_str())
        .is_some_and(|value| !value.trim().is_empty())
}

fn toml_has_credentials(path: &PathBuf) -> bool {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return false,
    };
    let doc = match content.parse::<DocumentMut>() {
        Ok(doc) => doc,
        Err(_) => return false,
    };

    let paired_env = paired_env_path(path);
    let paired_ref = paired_env.is_file().then_some(paired_env.as_path());
    let is_config = path.file_name().and_then(|name| name.to_str()) == Some(CONFIG_FILENAME);

    if credential_field_is_set(doc.get("api_key"), paired_ref) {
        return true;
    }

    for spec in PROVIDER_SPECS {
        if credential_field_is_set(doc.get(spec.config_key), paired_ref) {
            return true;
        }
    }

    if is_config {
        let provider = doc
            .get("provider")
            .and_then(|item| item.as_str())
            .and_then(|name| Provider::from_str(name).ok());

        return provider == Some(Provider::Custom) && item_as_non_empty_str(doc.get("base_url"));
    }

    false
}

fn has_credentials() -> bool {
    if env::var("API_KEY").is_ok_and(|value| !value.trim().is_empty()) {
        return true;
    }
    if PROVIDER_SPECS.iter().any(|spec| {
        spec.env_var
            .is_some_and(|var| env::var(var).is_ok_and(|value| !value.trim().is_empty()))
    }) {
        return true;
    }

    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    config_search_paths(&cwd)
        .iter()
        .any(|path| toml_has_credentials(path))
}
