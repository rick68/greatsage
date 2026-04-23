use std::process::Command;

/// Stage all changes in the current repository.
///
/// Returns `Ok(())` on success or an `Err` containing the error output.
pub fn stage_all() -> Result<(), String> {
    let output = Command::new("git")
        .args(&["add", "."])
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        // Prefer stderr, fallback to stdout if stderr empty
        let mut err = String::from_utf8_lossy(&output.stderr).into_owned();
        if err.trim().is_empty() {
            err = String::from_utf8_lossy(&output.stdout).into_owned();
        }
        Err(err.trim().to_string())
    }
}

/// Commit staged changes with the given message.
///
/// Returns `Ok(())` on success or an `Err` containing the error output.
pub fn commit(message: &str) -> Result<(), String> {
    let output = Command::new("git")
        .args(&["commit", "-m", message])
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        // Prefer stderr, fallback to stdout if stderr empty
        let mut err = String::from_utf8_lossy(&output.stderr).into_owned();
        if err.trim().is_empty() {
            err = String::from_utf8_lossy(&output.stdout).into_owned();
        }
        Err(err.trim().to_string())
    }
}

/// Revert the most recent commit.
///
/// Returns `Ok(())` on success or an `Err` containing the error output.
pub fn revert_last() -> Result<(), String> {
    // Try a simple revert; if it fails, the caller can decide alternative actions.
    let output = Command::new("git")
        .args(&["revert", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        // Prefer stderr, fallback to stdout if stderr empty
        let mut err = String::from_utf8_lossy(&output.stderr).into_owned();
        if err.trim().is_empty() {
            err = String::from_utf8_lossy(&output.stdout).into_owned();
        }
        Err(err.trim().to_string())
    }
}
