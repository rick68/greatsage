//! Short path labels for REPL status lines.
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.

use std::{
    env,
    path::{Path, PathBuf},
};

/// Short path for REPL status lines (strip cwd or `~`, else basename).
fn display_session_path(path: impl AsRef<Path>) -> String {
    if let Ok(cwd) = env::current_dir()
        && let Ok(rel) = path.as_ref().strip_prefix(&cwd)
    {
        let rel = rel.display().to_string();
        if !rel.is_empty() {
            return rel;
        }
    }

    if let Ok(home) = env::var("HOME")
        && let Ok(rel) = path.as_ref().strip_prefix(PathBuf::from(home))
    {
        return if rel.as_os_str().is_empty() {
            String::from("~")
        } else {
            format!("~/{}", rel.display())
        };
    }

    path.as_ref()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.as_ref().display().to_string())
}

/// Path label for save/load status lines: default session file shows basename only.
pub(super) fn session_status_path(path: impl AsRef<Path>, default: impl AsRef<Path>) -> String {
    if path.as_ref() == default.as_ref() {
        path.as_ref()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| display_session_path(path))
    } else {
        display_session_path(path)
    }
}
