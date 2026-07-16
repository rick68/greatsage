//! Per-provider OAuth token persistence under the user config directory.

use {
    crate::config_paths::user_config_dir,
    serde::{Deserialize, Serialize},
    std::{
        fs, io,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    },
    thiserror::Error,
};

const TOKENS_SUBDIR: &str = "tokens";

/// Stored OAuth tokens for one provider.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StoredTokens {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Unix seconds when the access token expires (if known).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    pub updated_at: u64,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("token file is world-readable; refusing to load ({0})")]
    WorldReadable(PathBuf),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Directory for token files: `~/.config/greatsage/tokens` (honors `XDG_CONFIG_HOME`).
pub fn tokens_dir() -> PathBuf {
    user_config_dir().join(TOKENS_SUBDIR)
}

/// Path for one provider's token file (`tokens/<provider_id>.json`).
pub fn token_path(provider_id: &str) -> PathBuf {
    tokens_dir().join(format!("{provider_id}.json"))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl StoredTokens {
    pub fn new(
        access_token: impl Into<String>,
        refresh_token: Option<String>,
        expires_in: Option<u64>,
        token_type: Option<String>,
        scope: Option<String>,
    ) -> Self {
        let updated_at = now_unix();
        let expires_at = expires_in.map(|secs| updated_at.saturating_add(secs));
        Self {
            access_token: access_token.into(),
            refresh_token,
            expires_at,
            token_type,
            scope,
            updated_at,
        }
    }

    /// True when `expires_at` is set and is at or before `now` (with 30s skew).
    pub fn access_expired(&self) -> bool {
        match self.expires_at {
            None => false,
            Some(exp) => {
                let now = now_unix().saturating_add(30);
                now >= exp
            }
        }
    }

    pub fn has_usable_fields(&self) -> bool {
        !self.access_token.trim().is_empty()
            || self
                .refresh_token
                .as_ref()
                .is_some_and(|t| !t.trim().is_empty())
    }
}

/// Save tokens for `provider_id` with mode 0600 on Unix.
pub fn save_tokens(provider_id: &str, tokens: &StoredTokens) -> Result<(), StoreError> {
    let dir = tokens_dir();
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dir)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&dir, perms)?;
    }

    let path = token_path(provider_id);
    let json = serde_json::to_vec_pretty(tokens)?;
    () = write_private(&path, &json)?;
    Ok(())
}

/// Load tokens; refuses world-readable files on Unix.
pub fn load_tokens(provider_id: &str) -> Result<Option<StoredTokens>, StoreError> {
    let path = token_path(provider_id);
    if !path.is_file() {
        return Ok(None);
    }
    () = refuse_world_readable(&path)?;
    let data = fs::read_to_string(&path)?;
    let tokens: StoredTokens = serde_json::from_str(&data)?;
    Ok(Some(tokens))
}

/// Delete the store entry for `provider_id` (no-op if missing).
pub fn delete_tokens(provider_id: &str) -> Result<(), StoreError> {
    let path = token_path(provider_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// True if any `tokens/*.json` has a non-empty access or refresh token (no network).
pub fn any_oauth_tokens_present() -> bool {
    let dir = tokens_dir();
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if refuse_world_readable(&path).is_err() {
            continue;
        }
        let Ok(data) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(tokens) = serde_json::from_str::<StoredTokens>(&data) else {
            continue;
        };
        if tokens.has_usable_fields() {
            return true;
        }
    }
    false
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)?;
        let mut perms = file.metadata()?.permissions();
        () = perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    {
        fs::write(path, bytes)?;
    }
    Ok(())
}

fn refuse_world_readable(path: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)?.permissions().mode();
        // Refuse if group or other can read.
        if mode & 0o044 != 0 {
            return Err(StoreError::WorldReadable(path.to_path_buf()));
        }
    }
    let _ = path;
    Ok(())
}
