use {
    std::{
        fs::{self, File},
        future::Future,
        io::{ErrorKind, Write},
        path::Path,
        pin::Pin,
        time::Duration,
    },
    tokio::{fs as async_fs, runtime::Runtime, time},
};

/// Returns true if the given path is within a protected location that
/// should not be modified by the evolve pipeline.
fn is_protected_path(path: impl AsRef<Path>) -> bool {
    // Define protected paths relative to the repository root.
    // We treat both files and directories uniformly.
    let protected = [
        Path::new(".github/workflows"),
        Path::new("IDENTITY.md"),
        Path::new("scripts"),
        Path::new("skills"),
    ];

    // Convert the path into its components for precise matching.
    // This avoids false positives where a protected name appears as a
    // substring of a different component (e.g., "scripts_backup").
    let components: Vec<_> = path.as_ref().components().map(|c| c.as_os_str()).collect();

    for prot in &protected {
        let prot_comps: Vec<_> = prot.components().map(|c| c.as_os_str()).collect();
        let prot_len = prot_comps.len();
        if prot_len == 0 {
            continue;
        }
        // Single‑component protection (e.g., "scripts", "skills", "IDENTITY.md")
        // matches if any component equals it.
        if prot_len == 1 {
            if components.iter().any(|c| *c == prot_comps[0]) {
                return true;
            }
            continue;
        }
        // Multi‑component protection (e.g., ".github/workflows") matches if the
        // sequence of components appears consecutively anywhere in the path.
        if components.len() >= prot_len {
            for start in 0..=components.len() - prot_len {
                if components[start..start + prot_len] == prot_comps[..] {
                    return true;
                }
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

/// Orchestrates the evolve pipeline: assessment, planning, and task execution.
pub fn run_evolve_with(base_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let assessment = assessment_phase(&base_dir)?;
    println!("[greatsage] Assessment Phase Result:\n{assessment}");
    // Planning phase
    () = planning_phase(&base_dir)?;
    println!("[greatsage] Planning Phase completed. Task files created in session_plan/.");
    // Execute tasks phase
    () = execute_tasks(&base_dir)?;
    println!("[greatsage] Execute Tasks Phase completed. Log written to evolve.log.");
    Ok(())
}

/// Execute the task execution phase.
/// Reads markdown files in `session_plan/`, parses the `Title:` line,
/// prints a message for each task, and logs the execution to `evolve.log`.
/// Only processes files ending with `.md` and aborts if any path is protected.
fn execute_tasks(base_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let plan_dir = base_dir.as_ref().join("session_plan");
    // Guard against protected paths.
    let canonical_plan_dir = fs::canonicalize(&plan_dir).unwrap_or(plan_dir.clone());
    if is_protected_path(&canonical_plan_dir) {
        return Err(Box::new(std::io::Error::other(format!(
            "execute_tasks aborted: protected path {}",
            canonical_plan_dir.display()
        ))));
    }

    // Open (or create) the evolve.log file under .greatsage/.
    let log_dir = base_dir.as_ref().join(".greatsage");
    fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("evolve.log");
    let mut log_file = File::create(&log_path)?;

    // Iterate over markdown files in the session_plan directory.
    for entry in fs::read_dir(&plan_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            // Ensure the individual task file is not a protected path.
            let canonical_task_path = fs::canonicalize(&path).unwrap_or(path.clone());
            if is_protected_path(&canonical_task_path) {
                return Err(Box::new(std::io::Error::other(format!(
                    "execute_tasks aborted: protected task file {}",
                    canonical_task_path.display()
                ))));
            }
            let content = fs::read_to_string(&path)?;
            // Find the Title line.
            for line in content.lines() {
                if line.starts_with("Title:") {
                    let title = line.trim_start_matches("Title:").trim();
                    println!("Executing task: {title}");
                    writeln!(log_file, "Executing task: {title}")?;
                    break;
                }
            }
        }
    }
    Ok(())
}

pub fn planning_phase(base_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    // Create session_plan directory
    let plan_dir = base_dir.as_ref().join("session_plan");
    // Resolve canonical path to detect protected locations even via symlinks
    let canonical_plan_dir = fs::canonicalize(&plan_dir).unwrap_or(plan_dir.clone());
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
            fs::canonicalize(&file_path).unwrap_or(file_path.clone());
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
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let base = tmp.path();
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
    fn test_execute_tasks() {
        // Setup temporary base directory with a session_plan and a task file.
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let base = tmp.path();
        let plan_dir = base.join("session_plan");
        () = fs::create_dir_all(&plan_dir).expect("create session_plan dir");
        let task_path = plan_dir.join("task_01.md");
        () = fs::write(&task_path, "Title: Sample Task\nDetails: none\n").expect("write task file");
        // Execute tasks phase.
        () = execute_tasks(base).expect("execute_tasks should succeed");
        // Verify evolve.log contains the task title.
        let log_path = base.join(".greatsage").join("evolve.log");
        assert!(log_path.is_file(), "evolve.log should be created");
        let log_content = fs::read_to_string(&log_path).expect("read evolve.log");
        assert!(
            log_content.contains("Sample Task"),
            "log should contain task title"
        );
    }
}
