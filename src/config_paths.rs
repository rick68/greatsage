use {
    std::{
        fs,
        path::{Path, PathBuf},
    },
    toml_edit::DocumentMut,
};

pub const PROJECT_DIR: &str = ".greatsage";
pub const CONFIG_FILENAME: &str = "config.toml";
pub const ENV_FILENAME: &str = ".env";

/// Project config: `.greatsage/config.toml`
pub fn project_config_path(cwd: &Path) -> PathBuf {
    cwd.join(PROJECT_DIR).join(CONFIG_FILENAME)
}

/// User XDG config: `~/.config/greatsage/config.toml`
pub fn user_config_path(home: &Path) -> PathBuf {
    home.join(".config").join("greatsage").join(CONFIG_FILENAME)
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

/// Config search order (first populated file wins): project (walk-up) → user.
pub fn config_search_paths(cwd: &Path, home: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(project) = find_populated_project_config(cwd) {
        paths.push(project);
    }
    () = paths.push(user_config_path(home));
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

/// User XDG env: `~/.config/greatsage/.env`
pub fn user_env_path(home: &Path) -> PathBuf {
    home.join(".config").join("greatsage").join(ENV_FILENAME)
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
fn find_existing_file_upward(
    start: impl AsRef<Path>,
    path_at: impl Fn(&Path) -> PathBuf,
) -> Option<PathBuf> {
    let mut dir = start.as_ref().to_path_buf();
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
pub fn env_file_search_paths_low_to_high(cwd: &Path, home: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let user = user_env_path(home);
    if user.is_file() {
        () = paths.push(user);
    }
    if let Some(project) = find_existing_file_upward(cwd, project_env_path) {
        () = paths.push(project);
    }
    if let Some(repo) = find_existing_file_upward(cwd, repo_env_path) {
        if !paths.contains(&repo) {
            () = paths.push(repo);
        }
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
