mod coding;
pub use coding::{CodingAgentPromptChannel, CodingAgentTask};

use {
    self::coding::coding_agent_plugin,
    crate::tokio::AppCancelToken,
    backon::{BlockingRetryable, ConstantBuilder},
    bevy::{
        app::{App, Startup},
        ecs::{
            change_detection::{Res, ResMut},
            resource::Resource,
            system::IsFunctionSystem,
        },
        prelude::Deref,
    },
    bevy_tokio_tasks::{TaskContext, TokioTasksRuntime},
    std::{cell::Cell, env, path::PathBuf, sync::Arc, time::Duration},
    tokio::task::JoinHandle,
    tokio_util::sync::CancellationToken,
};

/// Maximum number of retry attempts for LLM requests.
pub const MAX_RETRY_ATTEMPTS: usize = 3;

/// Retry a closure up to `MAX_RETRY_ATTEMPTS` times.
/// Logs each attempt via `eprintln!`.
#[allow(dead_code)]
pub fn retry<T, E, F>(mut f: F) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let builder: ConstantBuilder = ConstantBuilder::new()
        .with_delay(Duration::ZERO)
        .with_max_times(MAX_RETRY_ATTEMPTS);
    // Track attempt count using a `Cell` (interior mutable) without unsafe or Rc.
    let attempt: Cell<usize> = Cell::new(0);
    (|| -> Result<T, E> {
        // Increment attempt count before each try.
        attempt.set(attempt.get() + 1);
        f()
    })
    .retry(builder)
    .notify::<_>(|_: &E, _: Duration| {
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
    Fut: std::future::Future<Output = Result<T, ()>>,
{
    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        match f().await {
            ok @ Ok(_) => return ok,
            Err(_) => {
                eprintln!(
                    "LLM request failed on attempt {}/{}; retrying...",
                    attempt, MAX_RETRY_ATTEMPTS
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

#[derive(Resource)]
pub struct LlmConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            base_url: dotenvy::var::<&str>("BASE_URL").unwrap_or_default(),
            model: dotenvy::var::<&str>("MODEL").unwrap_or_default(),
            api_key: dotenvy::var::<&str>("API_KEY").unwrap_or_default(),
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
    pub fn is_path_allowed(&self, path: &std::path::Path) -> bool {
        // Canonicalize both paths to handle relative components
        if let Ok(canonical_allowed) = self.allowed_dir.canonicalize()
            && let Ok(canonical_target) = path.canonicalize()
        {
            canonical_target.starts_with::<&PathBuf>(&canonical_allowed)
        } else {
            false
        }
    }

    /// Validate a given string path against the permission config.
    /// Returns Ok(()) if allowed, otherwise Err with a human‑readable message.
    pub fn validate_path(&self, path_str: &str) -> Result<(), String> {
        // Treat the input as a filesystem path; if it cannot be canonicalized (e.g., a command
        // string for the `bash` tool), we consider it allowed.
        let path = std::path::Path::new(path_str);
        match path.canonicalize() {
            Ok(canonical_target) => {
                if self.is_path_allowed(&canonical_target) {
                    Ok(())
                } else {
                    Err(format!("Permission denied for path: {}", path_str))
                }
            }
            Err(_e) => {
                // Not a valid path (likely a command) – allow.
                Ok(())
            }
        }
    }
}

impl Default for PermissionConfig {
    fn default() -> Self {
        let cwd: PathBuf = env::current_dir().unwrap_or_else::<fn(std::io::Error) -> PathBuf>(
            |_: std::io::Error| -> PathBuf { PathBuf::from(".") },
        );

        Self { allowed_dir: cwd }
    }
}

fn setup(
    app_cancel: Res<'_, AppCancelToken>,
    agents_cancel: Res<'_, AgentsCancelToken>,
    tokio_runtime: ResMut<'_, TokioTasksRuntime>,
) {
    let app_cancel: Arc<CancellationToken> = app_cancel.clone();
    let agents_cancel: Arc<CancellationToken> = agents_cancel.clone();

    let _: JoinHandle<()> =
        tokio_runtime.spawn_background_task::<_, (), _>(|_ctx: TaskContext| async move {
            tokio::select! {
                _ = app_cancel.cancelled() => (),
                _ = agents_cancel.cancelled() => (),
                else => unreachable!(),
            }
        });
}

pub fn agents_plugin(app: &mut App) {
    let _: &mut App = app
        .init_resource::<LlmConfig>()
        .init_resource::<AgentsCancelToken>()
        .init_resource::<PermissionConfig>()
        .add_plugins::<_>(coding_agent_plugin)
        .add_systems::<(
            IsFunctionSystem,
            fn(
                _, // Res<'_, AppCancelToken>
                _, // Res<'_, AgentsCancelToken>
                _, // ResMut<'_, TokioTasksRuntime>
            ) -> (),
        )>(Startup, setup);
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        pretty_assertions::assert_eq,
        tempfile::{TempDir, tempdir},
    };

    #[test]
    fn test_retry_success_on_second_attempt() {
        let mut call_count: usize = 0;
        let result: Result<i32, ()> = retry::<i32, (), _>(|| {
            call_count += 1;
            if call_count == 1 { Err(()) } else { Ok(42) }
        });
        assert_eq!(result, Ok(42));
        assert_eq!(call_count, 2);
    }

    #[test]
    fn permission_allows_path_within_cwd() {
        // Create a temporary directory that will serve as the allowed base.
        let allowed_dir: TempDir = tempdir().expect("failed to create temp dir");
        let allowed_path: PathBuf = allowed_dir.path().to_path_buf();
        // Create a sub-file inside the allowed directory.
        let sub_path: PathBuf = allowed_path.join("sub.txt");
        () = std::fs::write::<&PathBuf, &[u8]>(&sub_path, b"test")
            .expect("failed to write sub file");

        let perm: PermissionConfig = PermissionConfig {
            allowed_dir: allowed_path.clone(),
        };
        assert!(perm.validate_path(sub_path.to_str().unwrap()).is_ok());
    }

    #[test]
    fn permission_denies_path_outside_cwd() {
        // Allowed directory
        let allowed_dir: TempDir = tempdir().expect("failed to create allowed temp dir");
        let allowed_path: PathBuf = allowed_dir.path().to_path_buf();
        // Separate directory not allowed
        let denied_dir: TempDir = tempdir().expect("failed to create denied temp dir");
        let denied_path: PathBuf = denied_dir.path().join("outside.txt");
        () = std::fs::write(&denied_path, b"nope").expect("failed to write denied file");

        let perm = PermissionConfig {
            allowed_dir: allowed_path.clone(),
        };
        let result: Result<(), String> = perm.validate_path(denied_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Permission denied"));
    }
}
