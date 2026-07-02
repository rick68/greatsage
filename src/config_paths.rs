use {
    std::{
        env, fs,
        path::{Path, PathBuf},
    },
    toml_edit::DocumentMut,
};

pub const PROJECT_DIR: &str = ".greatsage";
pub const CONFIG_FILENAME: &str = "config.toml";
pub const ENV_FILENAME: &str = ".env";
const APP_CONFIG_SUBDIR: &str = "greatsage";

/// Config base directory — always `~/.config` unless `XDG_CONFIG_HOME` is set (Linux/macOS parity).
pub fn xdg_config_dir() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|path| Path::new(path).is_absolute())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        })
}

/// User-level greatsage directory (`~/.config/greatsage`; honors `XDG_CONFIG_HOME` when set).
pub fn user_config_dir() -> PathBuf {
    xdg_config_dir().join(APP_CONFIG_SUBDIR)
}

/// User config: `~/.config/greatsage/config.toml` (honors `XDG_CONFIG_HOME` when set).
pub fn user_config_path() -> PathBuf {
    user_config_dir().join(CONFIG_FILENAME)
}

/// Legacy macOS path (`~/Library/Application Support/greatsage/config.toml`) for migration only.
pub fn legacy_platform_config_path() -> Option<PathBuf> {
    let legacy = dirs::config_dir()?
        .join(APP_CONFIG_SUBDIR)
        .join(CONFIG_FILENAME);
    if legacy == user_config_path() {
        None
    } else {
        Some(legacy)
    }
}

/// REPL prompt input history (readline-style ↑↓ recall).
pub fn repl_history_path() -> PathBuf {
    user_config_dir().join("history")
}

/// Project config: `.greatsage/config.toml`
pub fn project_config_path(cwd: &Path) -> PathBuf {
    cwd.join(PROJECT_DIR).join(CONFIG_FILENAME)
}

/// Nearest populated `.greatsage/config.toml` walking up from `start` (includes `start`).
pub fn find_populated_project_config(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = project_config_path(&dir);
        if config_file_is_populated(&candidate) {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Resolved `config.toml` for `cwd` — same selection order as `Config::config_file()`.
pub fn resolved_config_path(cwd: &Path) -> PathBuf {
    for path in config_search_paths(cwd) {
        if config_file_is_populated(&path) {
            return path.canonicalize().unwrap_or(path);
        }
    }

    let default = user_config_path();
    if let Some(parent) = default.parent() {
        let _ = fs::create_dir_all(parent);
        let _ = fs::File::create(&default);
    }

    default.canonicalize().unwrap_or(default)
}

/// Display path for startup `config:` hint (relative to `cwd` when possible).
pub fn display_config_path(config_path: &Path, cwd: &Path) -> String {
    if let Ok(rel) = config_path.strip_prefix(cwd) {
        let rel = rel.display().to_string();
        if !rel.is_empty() {
            return rel;
        }
    }

    if let Ok(home) = env::var("HOME")
        && let Ok(rel) = config_path.strip_prefix(PathBuf::from(home))
    {
        return if rel.as_os_str().is_empty() {
            String::from("~")
        } else {
            format!("~/{}", rel.display())
        };
    }

    config_path.display().to_string()
}

/// One dimmed startup line: `  config: {path}`.
pub fn config_hint_line(config_path: &Path, cwd: &Path) -> String {
    format!("  config: {}", display_config_path(config_path, cwd))
}

/// Config search order (first populated file wins): project (walk-up) → `~/.config` user → legacy macOS.
pub fn config_search_paths(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(project) = find_populated_project_config(cwd) {
        () = paths.push(project);
    }
    () = paths.push(user_config_path());
    if let Some(legacy) = legacy_platform_config_path() {
        () = paths.push(legacy);
    }
    paths
}

/// True when the file exists and contains a non-empty TOML document.
pub fn config_file_is_populated(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if meta.len() == 0 {
        return false;
    }
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    if content.trim().is_empty() {
        return false;
    }
    content
        .parse::<DocumentMut>()
        .ok()
        .is_some_and(|doc| !doc.as_table().is_empty())
}

/// User env: `~/.config/greatsage/.env` (via `xdg_config_dir()`)
pub fn user_env_path() -> PathBuf {
    user_config_dir().join(ENV_FILENAME)
}

/// Project env: `.greatsage/.env`
pub fn project_env_path(cwd: &Path) -> PathBuf {
    cwd.join(PROJECT_DIR).join(ENV_FILENAME)
}

/// Repo-root env: `./.env` (relative to a directory).
pub fn repo_env_path(cwd: &Path) -> PathBuf {
    cwd.join(ENV_FILENAME)
}

/// Outermost existing file found walking up from `start` (includes `start`).
fn find_existing_file_upward(start: &Path, path_at: impl Fn(&Path) -> PathBuf) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    let mut found = None;
    loop {
        let candidate = path_at(&dir);
        if candidate.is_file() {
            found = Some(candidate);
        }
        if !dir.pop() {
            return found;
        }
    }
}

/// `.env` files from lowest to highest precedence (later entries override earlier).
///
/// Order: `~/.config/greatsage/.env` → `.greatsage/.env` (walk-up) → `./.env` (walk-up).
/// Shell environment set before startup is never overwritten.
pub fn env_file_search_paths_low_to_high(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let user = user_env_path();
    if user.is_file() {
        () = paths.push(user);
    }
    if let Some(project) = find_existing_file_upward(cwd, project_env_path) {
        () = paths.push(project);
    }
    if let Some(repo) = find_existing_file_upward(cwd, repo_env_path)
        && !paths.contains(&repo)
    {
        () = paths.push(repo);
    }
    paths
}

/// Paired `.env` for a `config.toml` path (same parent directory).
pub fn paired_env_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(|dir| dir.join(ENV_FILENAME))
        .unwrap_or_else(|| PathBuf::from(ENV_FILENAME))
}
