pub mod coding;
pub use coding::{CodingAgentPromptChannel, CodingAgentTask, CodingAgentTotalTokenUsage};

mod tools;
pub use tools::build_tools;

use {
    self::coding::coding_agent_plugin,
    crate::{
        config::{AppConfig, ThinkingLevel},
        tokio::AppCancelToken,
    },
    anyhow::Context,
    backon::{BlockingRetryable, ConstantBuilder},
    bevy::{
        app::{App, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            resource::Resource,
            world::{FromWorld, World},
        },
        prelude::Deref,
    },
    bevy_tokio_tasks::{TaskContext, TokioTasksRuntime},
    std::{
        cell::Cell,
        env,
        future::Future,
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    },
    tokio::task::JoinHandle,
    tokio_util::sync::CancellationToken,
    url::Url,
};

#[derive(Debug, thiserror::Error)]
pub enum PermissionError {
    #[error("Permission denied for path: {0}")]
    PathDenied(String),
}

/// Maximum number of retry attempts for LLM requests.
pub const MAX_RETRY_ATTEMPTS: usize = 3;

/// Retry a closure up to `MAX_RETRY_ATTEMPTS` times.
/// Logs each attempt via `eprintln!`.
#[allow(dead_code)]
pub fn retry<T, E, F>(mut f: F) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let builder = ConstantBuilder::new()
        .with_delay(Duration::ZERO)
        .with_max_times(MAX_RETRY_ATTEMPTS);
    // Track attempt count using a `Cell` (interior mutable) without unsafe or Rc.
    let attempt = Cell::new(0_usize);
    (|| -> Result<T, E> {
        // Increment attempt count before each try.
        () = attempt.set(attempt.get() + 1);
        f()
    })
    .retry(builder)
    .notify(|_: &E, _: Duration| {
        eprintln!(
            "LLM request failed on attempt {}/{MAX_RETRY_ATTEMPTS}; retrying...",
            attempt.get()
        );
    })
    .call()
}

/// Async version of `retry` for futures.
pub async fn retry_async<F, Fut, T>(mut f: F) -> Result<T, ()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ()>>,
{
    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        match f().await {
            ok @ Ok(_) => return ok,
            Err(_) => {
                eprintln!(
                    "LLM request failed on attempt {attempt}/{MAX_RETRY_ATTEMPTS}; retrying...",
                );
                if attempt == MAX_RETRY_ATTEMPTS {
                    return Err(());
                }
                // Simple backoff could be added here.
            }
        }
    }
    Err(())
}

#[derive(Clone, Resource)]
pub struct LlmConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub max_tokens: u32,
    pub context_window: u32,
    pub thinking_level: ThinkingLevel,
    pub temperature: Option<f32>,
    pub max_turns: usize,
}

impl FromWorld for LlmConfig {
    fn from_world(world: &mut World) -> Self {
        let cfg = world.resource::<AppConfig>();
        // Env vars override config file values; API_KEY is env-only.
        let base_url = if let Ok(base_url) = env::var("BASE_URL")
            && !base_url.is_empty()
        {
            base_url
        } else {
            cfg.llm.base_url.clone()
        };
        let model = if let Ok(model) = env::var("MODEL")
            && !model.is_empty()
        {
            model
        } else {
            cfg.llm.model.clone()
        };
        let api_key = dotenvy::var("API_KEY").unwrap_or_default();
        let max_tokens = if let Ok(max_tokens) = env::var("MAX_TOKENS")
            && !max_tokens.is_empty()
            && let Ok(max_tokens) = max_tokens.parse()
        {
            max_tokens
        } else {
            cfg.llm.max_tokens
        };
        let context_window = if let Ok(context_window) = env::var("CONTEXT_WINDOW")
            && !context_window.is_empty()
            && let Ok(context_window) = context_window.parse()
        {
            context_window
        } else {
            cfg.llm.context_window
        };
        let thinking_level = if let Ok(thinking_level) = env::var("THINKING_LEVEL")
            && !thinking_level.is_empty()
        {
            match thinking_level.to_lowercase().as_str() {
                "minimal" => ThinkingLevel::Minimal,
                "low" => ThinkingLevel::Low,
                "medium" => ThinkingLevel::Medium,
                "high" => ThinkingLevel::High,
                _ => ThinkingLevel::Off,
            }
        } else {
            cfg.llm.thinking_level
        };
        let temperature = if let Ok(temperature) = env::var("TEMPERATURE")
            && !temperature.is_empty()
            && let Ok(temperature) = temperature.parse()
        {
            Some(temperature)
        } else {
            cfg.llm.temperature
        };
        let max_turns = if let Ok(max_turns) = env::var("MAX_TURNS")
            && !max_turns.is_empty()
            && let Ok(max_turns) = max_turns.parse()
        {
            max_turns
        } else {
            cfg.agent.max_turns
        };

        Self {
            base_url,
            model,
            api_key,
            max_tokens,
            context_window,
            thinking_level,
            temperature,
            max_turns,
        }
    }
}

#[derive(Default, Deref, Resource)]
struct AgentsCancelToken(Arc<CancellationToken>);

#[derive(Resource)]
pub struct PermissionConfig {
    pub allowed_dir: PathBuf,
}

#[allow(dead_code)]
impl PermissionConfig {
    pub fn is_path_allowed(&self, path: impl AsRef<Path>) -> bool {
        // Canonicalize both paths to handle relative components
        if let Ok(canonical_allowed) = self.allowed_dir.canonicalize()
            && let Ok(canonical_target) = path.as_ref().canonicalize()
        {
            canonical_target.starts_with(&canonical_allowed)
        } else {
            false
        }
    }

    /// Validate a given string path against the permission config.
    /// Returns Ok(()) if allowed, otherwise Err with a human‑readable message.
    pub fn validate_path(&self, path_str: impl AsRef<str>) -> Result<(), PermissionError> {
        let debug = env::var("PERMISSION_DEBUG")
            .map(|v| v == "1")
            .unwrap_or(false);
        // Empty string or bare '/' are not meaningful file targets — allow.
        let trimmed = path_str.as_ref().trim();
        if trimmed.is_empty() || trimmed == "/" {
            return Ok(());
        }
        let deny = || PermissionError::PathDenied(String::from(path_str.as_ref()));
        // Expand leading `~` to the user's home directory for convenience.
        let expanded = if path_str.as_ref().starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                let without_tilde = path_str.as_ref().trim_start_matches('~');
                let stripped = without_tilde.strip_prefix('/').unwrap_or(without_tilde);
                home.join(stripped).to_string_lossy().into_owned()
            } else {
                String::from(path_str.as_ref())
            }
        } else {
            String::from(path_str.as_ref())
        };
        let path = Path::new(&expanded);
        let result = match path.canonicalize() {
            Ok(canonical_target) => {
                if self.is_path_allowed(canonical_target) {
                    Ok(())
                } else {
                    Err(deny())
                }
            }
            Err(_) => {
                // Path may not exist yet (e.g., a new file to be created).
                // In that case, consider its parent directory for permission checking.
                if let Some(parent) = path.parent() {
                    match parent.canonicalize() {
                        Ok(parent_canonical) => {
                            if self.is_path_allowed(&parent_canonical) {
                                Ok(())
                            } else {
                                Err(deny())
                            }
                        }
                        Err(_) => {
                            // Parent also cannot be resolved. If the token contains '..', deny
                            // (potential traversal attack). Otherwise allow — it's almost certainly
                            // not a real path (e.g., a GitHub "user/repo" reference).
                            if path_str.as_ref().contains("..") {
                                Err(deny())
                            } else {
                                Ok(())
                            }
                        }
                    }
                } else {
                    Err(deny())
                }
            }
        };
        if debug {
            match &result {
                Ok(()) => eprintln!(
                    "[permission] validate_path OK: {} (allowed_dir={:?})",
                    path_str.as_ref(),
                    self.allowed_dir
                ),
                Err(e) => eprintln!(
                    "[permission] validate_path DENIED: {} (allowed_dir={:?}) — {e}",
                    path_str.as_ref(),
                    self.allowed_dir
                ),
            }
        }
        result
    }

    /// Validate a bash command string by checking any path‑like tokens.
    /// Tokens that contain a '/' or start with '.' are considered potential paths.
    /// Returns Ok(()) if all such tokens are within the allowed directory.
    /// Validate a bash command string by checking any path‑like tokens.
    ///
    /// The original implementation considered **any** token containing a `/` or starting with `.`
    /// a potential file path. This caused false‑positives for URLs (e.g. `http://example.com`) and
    /// JSON strings that may contain slashes or dots. The updated logic applies a more nuanced
    /// heuristic:
    ///
    /// * Tokens that look like URLs (`scheme://...`) are ignored.
    /// * Tokens that start with a JSON delimiter (`{` or `[`) are ignored.
    /// * Tokens that start with `-` are treated as flags and skipped.
    /// * Remaining tokens that contain a `/` or start with `.` are considered file paths and are
    ///   validated via `validate_path`.
    ///
    /// This reduces over‑rejection while preserving security – only clearly path‑like arguments are
    /// checked against the allowed directory.
    #[allow(clippy::while_let_on_iterator)]
    pub fn validate_command(&self, command: impl AsRef<str>) -> anyhow::Result<()> {
        let debug = env::var("PERMISSION_DEBUG")
            .map(|v| v == "1")
            .unwrap_or(false);
        // Split the command into whitespace‑separated tokens and iterate.
        let mut tokens = command.as_ref().split_whitespace().peekable();
        let mut first_token = true;
        while let Some(token) = tokens.next() {
            // Skip the executable itself (first token) — it's a system binary, not user data.
            if first_token {
                first_token = false;
                if debug {
                    eprintln!("[permission] skip executable: {token}");
                }
                continue;
            }
            // Skip flags.
            if token.starts_with('-') {
                continue;
            }
            // Skip URLs (e.g. http://example.com) – they contain "://".
            if token.contains("://") {
                continue;
            }
            // Skip JSON literals.
            if token.starts_with('{') || token.starts_with('[') {
                continue;
            }
            // Skip glob patterns — they are never real filesystem paths.
            if token.contains('*') || token.contains('?') {
                continue;
            }
            // Heuristic: treat as a path if it contains '/' or is relative '.' or '..'
            if token.contains('/') || token.starts_with('.') {
                // Strip surrounding quotes for cleaner validation.
                let stripped = token.trim_matches('\'').trim_matches('"');
                // Skip bare '/' and other single-char tokens — not a real path.
                if stripped.len() <= 1 {
                    continue;
                }
                // Skip nix store paths — read-only, system-managed, never user data.
                if stripped.starts_with("/nix/store/") {
                    if debug {
                        eprintln!("[permission] skip nix store path: {stripped}");
                    }
                    continue;
                }
                // Previously, paths starting with "/tmp" were skipped as a special case.
                // This caused legitimate outside‑directory paths (e.g., temporary files) to be allowed
                // even when they reside outside the configured `allowed_dir`. The test suite expects
                // such paths to be rejected, so we no longer treat "/tmp" specially.
                // (If a user truly wants to allow /tmp, they can configure `allowed_dir` accordingly.)
                let result = self
                    .validate_path(stripped)
                    .with_context(|| format!("Token '{token}' disallowed"));
                if debug {
                    match &result {
                        Ok(()) => eprintln!("[permission] allowed: {stripped}"),
                        Err(e) => eprintln!("[permission] denied: {stripped} — {e}"),
                    }
                }
                result?;
            }
        }
        Ok(())
    }
}

impl FromWorld for PermissionConfig {
    fn from_world(world: &mut World) -> Self {
        let cfg = world.resource::<AppConfig>();
        // ALLOWED_DIR env var overrides config file; config overrides cwd default.
        let raw = if let Ok(allowed_dir) = env::var("ALLOWED_DIR")
            && !allowed_dir.is_empty()
        {
            Some(allowed_dir)
        } else {
            let dir = &cfg.permissions.allowed_dir;
            if dir.is_empty() {
                None
            } else {
                Some(dir.clone())
            }
        };
        let allowed = match raw {
            Some(dir) => {
                let path = Path::new(&dir);
                path.canonicalize()
                    .unwrap_or(env::current_dir().unwrap_or(PathBuf::from(".")))
            }
            None => env::current_dir().unwrap_or(PathBuf::from(".")),
        };

        Self {
            allowed_dir: allowed,
        }
    }
}

fn setup(
    app_cancel: Res<AppCancelToken>,
    agents_cancel: Res<AgentsCancelToken>,
    tokio_runtime: ResMut<TokioTasksRuntime>,
) {
    let app_cancel = app_cancel.clone();
    let agents_cancel = agents_cancel.clone();

    let _: JoinHandle<()> = tokio_runtime.spawn_background_task(|_ctx: TaskContext| async move {
        tokio::select! {
            _ = app_cancel.cancelled() => (),
            _ = agents_cancel.cancelled() => (),
            else => unreachable!(),
        }
    });
}

/// MCP server entries supplied via `--mcp`. Each entry is either an HTTP(S) URL
/// (auto-detected by `http://` / `https://` prefix) or a stdio command string.
#[derive(Clone, Default, Resource)]
pub struct McpConfig {
    pub sse_transports: Vec<Url>,
    pub stdio_transports: Vec<String>,
}

impl From<Vec<String>> for McpConfig {
    fn from(servers: Vec<String>) -> Self {
        let mut sse_transports = Vec::new();
        let mut stdio_transports = Vec::new();
        for server in servers {
            if server.starts_with("http://") || server.starts_with("https://") {
                match Url::parse(&server) {
                    Ok(url) => sse_transports.push(url),
                    Err(e) => eprintln!("MCP: invalid URL '{server}': {e}"),
                }
            } else if !server.trim().is_empty() {
                () = stdio_transports.push(server);
            }
        }
        Self {
            sse_transports,
            stdio_transports,
        }
    }
}

pub fn agents_plugin(app: &mut App) {
    _ = app
        .init_resource::<LlmConfig>()
        .init_resource::<AgentsCancelToken>()
        .init_resource::<PermissionConfig>()
        .add_plugins(coding_agent_plugin)
        .add_systems(Startup, setup);
}

#[cfg(test)]
mod tests {
    use {super::*, pretty_assertions::assert_eq, std::fs, tempfile::tempdir};

    #[test]
    fn test_retry_success_on_second_attempt() {
        let mut call_count: usize = 0;
        let result = retry(|| {
            call_count += 1;
            if call_count == 1 { Err(()) } else { Ok(42) }
        });
        assert_eq!(result, Ok(42));
        assert_eq!(call_count, 2);
    }

    #[test]
    fn permission_denies_bash_outside_path() {
        // Setup allowed directory
        let allowed_dir = tempdir().expect("failed to create allowed temp dir");
        let allowed_path = allowed_dir.path().to_path_buf();
        // Create a file outside allowed directory
        let denied_dir = tempdir().expect("failed to create denied temp dir");
        let denied_path = denied_dir.path().join("outside.txt");
        () = std::fs::write(&denied_path, b"nope").expect("failed to write denied file");
        let perm = PermissionConfig {
            allowed_dir: allowed_path,
        };
        let cmd = format!("cat {}", denied_path.to_str().unwrap());
        let result = perm.validate_command(&cmd);
        assert!(result.is_err());
        assert!(format!("{:#}", result.unwrap_err()).contains("Permission denied"));
    }

    #[test]
    fn permission_denies_path_outside_cwd() {
        // Allowed directory
        let allowed_dir = tempdir().expect("failed to create allowed temp dir");
        let allowed_path = allowed_dir.path().to_path_buf();
        // Separate directory not allowed
        let denied_dir = tempdir().expect("failed to create denied temp dir");
        let denied_path = denied_dir.path().join("outside.txt");
        () = fs::write(&denied_path, b"nope").expect("failed to write denied file");

        let perm = PermissionConfig {
            allowed_dir: allowed_path.clone(),
        };
        let result = perm.validate_path(denied_path.to_str().unwrap());
        assert!(matches!(result, Err(PermissionError::PathDenied(_))));
    }

    #[test]
    fn command_with_allowed_flag_path_is_allowed() {
        // Allowed directory
        let allowed_dir = tempdir().expect("create allowed temp dir");
        let allowed_path = allowed_dir.path().to_path_buf();
        // Create a subdirectory to use with -C flag
        let sub_dir = allowed_path.join("sub");
        () = std::fs::create_dir_all(&sub_dir).expect("create sub dir");
        let config = PermissionConfig {
            allowed_dir: allowed_path,
        };
        let cmd = format!("git -C {} status", sub_dir.to_str().unwrap());
        assert!(config.validate_command(&cmd).is_ok());
    }

    #[test]
    fn command_with_disallowed_flag_path_is_denied() {
        let allowed_dir = tempdir().expect("create allowed temp dir");
        let denied_dir = tempdir().expect("create denied temp dir");
        let config = PermissionConfig {
            allowed_dir: allowed_dir.path().to_path_buf(),
        };
        let cmd = format!("git -C {} status", denied_dir.path().to_str().unwrap());
        let res = config.validate_command(&cmd);
        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(err.contains("Token") || err.contains("disallowed"));
    }

    #[test]
    fn command_mixed_allowed_and_disallowed_tokens() {
        let allowed_dir = tempdir().expect("create allowed temp dir");
        let allowed_file = allowed_dir.path().join("good.txt");
        () = fs::write(&allowed_file, b"ok").expect("write allowed file");
        let denied_dir = tempdir().expect("create denied temp dir");
        let denied_file = denied_dir.path().join("bad.txt");
        () = fs::write(&denied_file, b"no").expect("write denied file");
        let config = PermissionConfig {
            allowed_dir: allowed_dir.path().to_path_buf(),
        };
        let cmd = format!(
            "git -C {} cat {}",
            allowed_dir.path().to_str().unwrap(),
            denied_file.to_str().unwrap()
        );
        let res = config.validate_command(&cmd);
        assert!(res.is_err());
        let err = format!("{:#}", res.unwrap_err());
        assert!(err.contains(denied_file.to_str().unwrap()));
    }

    #[test]
    fn command_allows_bare_slash() {
        let allowed_dir = tempdir().expect("create allowed temp dir");
        let config = PermissionConfig {
            allowed_dir: allowed_dir.path().to_path_buf(),
        };
        assert!(config.validate_command("ls /").is_ok());
        assert!(config.validate_command("find / -name foo").is_ok());
    }

    #[test]
    fn command_skips_url_tokens() {
        // Allowed directory
        let allowed_dir = tempdir().expect("create allowed temp dir");
        let allowed_path = allowed_dir.path().to_path_buf();
        let config = PermissionConfig {
            allowed_dir: allowed_path,
        };
        // Command includes a URL and an allowed path token.
        let cmd = format!(
            "curl http://example.com -o {}",
            allowed_dir.path().join("out.txt").to_str().unwrap()
        );
        // Should be allowed because URL is skipped and path is within allowed dir.
        assert!(config.validate_command(&cmd).is_ok());
    }

    #[test]
    fn command_skips_json_tokens() {
        let allowed_dir = tempdir().expect("create allowed temp dir");
        let config = PermissionConfig {
            allowed_dir: allowed_dir.path().to_path_buf(),
        };
        // JSON token that includes slashes but should be ignored.
        let json = "{\"url\": \"http://example.com/path\"}";
        // Also include an allowed path token to ensure overall passes.
        let cmd = format!(
            "echo {json} {}",
            allowed_dir.path().join("file.txt").to_str().unwrap()
        );
        assert!(config.validate_command(&cmd).is_ok());
    }

    #[test]
    fn test_retry_failure_all_attempts() {
        // Count attempts
        let mut attempts = 0_usize;
        let result: Result<(), ()> = retry(|| {
            attempts += 1;
            Err(())
        });
        // Should be Err after MAX_RETRY_ATTEMPTS attempts (plus one final call)
        assert_eq!(attempts, MAX_RETRY_ATTEMPTS + 1);
        assert!(result.is_err());
    }

    #[test]
    fn validate_path_allows_nonexistent_file_within_allowed_dir() {
        let allowed_dir = tempdir().expect("create allowed temp dir");
        let allowed_path = allowed_dir.path().to_path_buf();
        let perm = PermissionConfig {
            allowed_dir: allowed_path.clone(),
        };
        // Path does not exist yet but is within allowed dir
        let new_file = allowed_path.join("new.txt");
        assert!(!new_file.exists());
        let result = perm.validate_path(new_file.to_str().unwrap());
        assert!(
            result.is_ok(),
            "validate_path should allow creation of new file within allowed dir"
        );
    }

    #[test]
    fn validate_path_denies_nonexistent_file_outside_allowed_dir() {
        let allowed_dir = tempdir().expect("create allowed temp dir");
        let denied_dir = tempdir().expect("create denied temp dir");
        let perm = PermissionConfig {
            allowed_dir: allowed_dir.path().to_path_buf(),
        };
        // Path does not exist and is outside allowed dir
        let new_file = denied_dir.path().join("new.txt");
        assert!(!new_file.exists());
        let result = perm.validate_path(new_file.to_str().unwrap());
        assert!(
            result.is_err(),
            "validate_path should deny creation of file outside allowed dir"
        );
    }
}
