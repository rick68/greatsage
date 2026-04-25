use {
    std::fs::{self, File},
    std::io::Write,
    std::{future::Future, io::ErrorKind, path::Path, pin::Pin, time::Duration},
    tokio::{fs as async_fs, runtime::Runtime, time},
};

/// Returns true if the given path is within a protected location that
/// should not be modified by the evolve pipeline.
fn is_protected_path(path: &Path) -> bool {
    // Define protected prefixes relative to the repository root.
    // We treat both files and directories uniformly.
    let protected = [
        Path::new(".github/workflows"),
        Path::new("IDENTITY.md"),
        Path::new("scripts"),
        Path::new("skills"),
    ];
    // Convert path to string for simple containment check.
    // This approach works for absolute paths as well, matching any segment.
    if let Some(s) = path.to_str() {
        for prot in &protected {
            if let Some(p) = prot.to_str()
                && s.contains(p)
            {
                return true;
            }
        }
    }
    false
}

const TIMEOUT_SECS: u64 = 1200;

/// Perform the Assessment Phase (A1) of the evolve pipeline.
/// Collects basic self‑analysis data such as version, source file count,
/// and a placeholder CI status. The operation is wrapped in a timeout of
/// half the total evolve timeout.
pub fn assessment_phase(base_dir: impl AsRef<Path>) -> Result<String, Box<dyn std::error::Error>> {
    let base_dir = base_dir.as_ref().to_path_buf();
    // Create a Tokio runtime to run the async timeout.
    let rt = Runtime::new()?;
    rt.block_on(async {
        // Wrap the actual assessment work in a timeout future.
        let work = async {
            let version = build_info::format!("{}", $.crate_info.version);

            // Count .rs source files recursively under src/.
            fn count_rs(
                dir: impl AsRef<Path> + Send + Sync + 'static,
            ) -> Pin<Box<dyn Future<Output = usize> + Send>> {
                Box::pin(async move {
                    let mut cnt = 0;
                    if let Ok(mut entries) = async_fs::read_dir(dir).await {
                        while let Ok(Some(entry)) = entries.next_entry().await {
                            let path = entry.path();
                            if path.is_dir() {
                                cnt += count_rs(path).await;
                            } else if path.extension().is_some_and(|e| e == "rs") {
                                cnt += 1;
                            }
                        }
                    }
                    cnt
                })
            }
            let src_files = count_rs(base_dir.join("src")).await;

            // Placeholder for latest CI status.
            let ci_status = "unknown";

            Ok::<String, Box<dyn std::error::Error>>(format!(
                "Version: {version}\nSource files: {src_files}\nCI last: {ci_status}"
            ))
        };

        match time::timeout(Duration::from_secs(TIMEOUT_SECS / 2), work).await {
            Ok(res) => res,
            Err(_) => Err(Box::new(std::io::Error::new(
                ErrorKind::TimedOut,
                "assessment phase timed out",
            )) as Box<dyn std::error::Error>),
        }
    })
}

pub fn run_evolve_with(base_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let assessment = assessment_phase(&base_dir)?;
    println!("[greatsage] Assessment Phase Result:\n{assessment}");
    // New: planning phase
    planning_phase(&base_dir)?;
    println!("[greatsage] Planning Phase completed. Task files created in session_plan/.");
    Ok(())
}

pub fn planning_phase(base_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    // Create session_plan directory
    let plan_dir = base_dir.as_ref().join("session_plan");
    // Resolve canonical path to detect protected locations even via symlinks
    let canonical_plan_dir = fs::canonicalize(&plan_dir).unwrap_or_else(|_| plan_dir.clone());
    if is_protected_path(&canonical_plan_dir) {
        return Err(Box::new(std::io::Error::other(format!(
            "planning_phase aborted: protected path {}",
            canonical_plan_dir.display()
        ))));
    }
    if plan_dir.exists() {
        // Clean existing task files
        for entry in fs::read_dir(&plan_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                fs::remove_file(path)?;
            }
        }
    } else {
        fs::create_dir_all(&plan_dir)?;
    }

    // Generate up to three placeholder tasks
    for i in 1..=3 {
        let file_path = plan_dir.join(format!("task_{:02}.md", i));
        // Additional safeguard: ensure each task file path is not protected
        let canonical_file_path =
            fs::canonicalize(&file_path).unwrap_or_else(|_| file_path.clone());
        if is_protected_path(&canonical_file_path) {
            return Err(Box::new(std::io::Error::other(format!(
                "planning_phase aborted: protected task file {}",
                canonical_file_path.display()
            ))));
        }
        let mut file = File::create(&file_path)?;
        writeln!(file, "Title: Placeholder Task {}", i)?;
        writeln!(file, "Files: none")?;
        writeln!(file, "Issue: none")?;
        writeln!(file, "\nGenerated by planning_phase.")?;
    }
    Ok(())
}

pub fn run_evolve() -> Result<(), Box<dyn std::error::Error>> {
    run_evolve_with(Path::new("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_assessment_phase_returns_nonempty() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"));
        let res = assessment_phase(base).expect("assessment_phase should succeed");
        assert!(
            !res.trim().is_empty(),
            "assessment result should not be empty"
        );
    }

    #[test]
    fn test_run_evolve_executes_without_error() {
        // Ensure that the evolve pipeline runs to completion without panicking.
        // The function prints to stdout; we only verify that it returns Ok.
        () = run_evolve_with(Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("run_evolve should complete without error");
    }

    #[test]
    fn test_planning_phase_creates_tasks() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"));
        // Clean any existing plan dir
        let _ = fs::remove_dir_all(base.join("session_plan"));
        () = planning_phase(base).expect("planning_phase should succeed");
        let plan_dir = base.join("session_plan");
        assert!(plan_dir.is_dir(), "session_plan directory should exist");
        for i in 1..=3 {
            let path = plan_dir.join(format!("task_{i:02}.md"));
            assert!(path.is_file(), "{} should exist", path.display());
            let content = fs::read_to_string(&path).expect("read task file");
            assert!(
                content.contains("Placeholder Task"),
                "Task file should contain placeholder title"
            );
        }
    }

    #[test]
    fn test_planning_phase_protected_path_fails() {
        // Create a temporary directory with a protected subdirectory 'scripts'
        let tmp_base = std::env::temp_dir().join("greatsage_test_protected");
        let protected_dir = tmp_base.join("scripts");
        // Ensure clean state
        let _ = fs::remove_dir_all(&tmp_base);
        fs::create_dir_all(&protected_dir).expect("create protected dir");
        // planning_phase should error because the plan directory would be inside a protected path
        let result = planning_phase(&protected_dir);
        assert!(
            result.is_err(),
            "planning_phase should reject protected path"
        );
        // Cleanup
        let _ = fs::remove_dir_all(&tmp_base);
    }
}
