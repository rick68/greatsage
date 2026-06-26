//! Layered `.env` loading and `env!VAR` credential references in TOML.
//!
//! Load order among files (low → high; shell env at startup wins over all files):
//! 1. `~/.config/greatsage/.env`
//! 2. `.greatsage/.env` (walk-up from cwd)
//! 3. `./.env` (walk-up from cwd)
//!
//! `load_layered_env()` merges these into the process environment before CLI parse.
//! Paired `.env` beside `config.toml` is also used as fallback for `env!VAR` resolution.

use {
    crate::config_paths::env_file_search_paths_low_to_high,
    std::{
        collections::{HashMap, HashSet},
        env, fs, io,
        path::Path,
    },
};

/// Parse a `.env` file into key/value pairs (`KEY="value"` and `KEY='value'` supported).
pub fn parse_env_file(path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(iter) = dotenvy::from_path_iter(path) else {
        return map;
    };
    for item in iter.flatten() {
        map.entry(item.0).or_insert(item.1);
    }
    map
}

/// Merge layered `.env` files into the process environment.
pub fn load_layered_env() {
    let frozen: HashSet<String> = env::vars().map(|(key, _)| key).collect();
    let cwd = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_default();

    let mut merged = HashMap::new();
    for path in env_file_search_paths_low_to_high(&cwd, &home) {
        for (key, value) in parse_env_file(&path) {
            merged.insert(key, value);
        }
    }

    for (key, value) in merged {
        if !frozen.contains(&key) {
            // SAFETY: called at process startup / after wizard before worker threads spawn.
            unsafe { env::set_var(&key, value) };
        }
    }
}

/// Format a config credential reference: `env!API_KEY` (no space after `!`).
pub fn format_env_reference(var_name: &str) -> String {
    format!("env!{var_name}")
}

/// If `value` is an `env!VAR` reference, return the variable name.
pub fn env_reference_name(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let name = trimmed.strip_prefix("env!")?;
    if name.is_empty()
        || name.starts_with(char::is_whitespace)
        || !name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some(name)
}

/// Resolve a TOML credential with an optional paired `.env` for `env!VAR` lookups.
pub fn resolve_credential_value_with_paired_env(
    value: &str,
    paired_env: Option<&Path>,
) -> Option<String> {
    if let Some(name) = env_reference_name(value.as_ref()) {
        if let Ok(shell) = env::var(name)
            && !shell.trim().is_empty()
        {
            return Some(shell);
        }
        if let Some(path) = paired_env.filter(|path| path.is_file()) {
            if let Some(file_value) = parse_env_file(path).get(name).cloned()
                && !file_value.trim().is_empty()
            {
                return Some(file_value);
            }
        }
        return None;
    }
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub fn credential_value_is_set_with_paired_env(value: &str, paired_env: Option<&Path>) -> bool {
    resolve_credential_value_with_paired_env(value, paired_env).is_some()
}

fn format_env_assignment(key: &str, value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '#' | '"' | '\'' | '\\' | '$' | '='));
    if needs_quotes {
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("{key}=\"{escaped}\"")
    } else {
        format!("{key}={value}")
    }
}

fn restrict_env_permissions(path: &Path) -> io::Result<()> {
    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        } else {
            Ok(())
        }
    }
}

fn env_line_key(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let trimmed = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    let (name, _) = trimmed.split_once('=')?;
    Some(name.trim())
}

/// Insert or update one `KEY=value` entry in a `.env` file.
pub fn upsert_env_file(path: &Path, key: &str, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        () = fs::create_dir_all(parent)?;
    }

    let mut lines: Vec<String> = if path.is_file() {
        fs::read_to_string(path)?
            .lines()
            .map(String::from)
            .collect()
    } else {
        Vec::new()
    };

    let assignment = format_env_assignment(key, value);
    let mut found = false;
    for line in &mut lines {
        if env_line_key(line) == Some(key) {
            *line = assignment.clone();
            found = true;
            break;
        }
    }
    if !found {
        () = lines.push(assignment);
    }

    let mut content = lines.join("\n");
    if !content.ends_with('\n') {
        () = content.push('\n');
    }
    () = fs::write(path, content)?;
    restrict_env_permissions(path.as_ref())
}
