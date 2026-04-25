use {
    std::{io::ErrorKind, path::Path, time::Duration},
    tokio::{fs, runtime::Runtime, time},
};

const TIMEOUT_SECS: u64 = 1200;

/// Perform the Assessment Phase (A1) of the evolve pipeline.
/// Collects basic self‑analysis data such as version, source file count,
/// and a placeholder CI status. The operation is wrapped in a timeout of
/// half the total evolve timeout.
pub fn assessment_phase(base_dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let base_dir = base_dir.to_path_buf();
    // Create a Tokio runtime to run the async timeout.
    let rt = Runtime::new()?;
    rt.block_on(async {
        // Wrap the actual assessment work in a timeout future.
        let work = async {
            // Read Cargo.toml version.
            let cargo_toml = fs::read_to_string(base_dir.join("Cargo.toml")).await?;
            let version_line = cargo_toml
                .lines()
                .find(|l| l.trim_start().starts_with("version"))
                .ok_or("Version not found in Cargo.toml")?;
            let version = version_line
                .split('"')
                .nth(1)
                .unwrap_or("unknown")
                .to_string();

            // Count .rs source files recursively under src/.
            fn count_rs(
                dir: std::path::PathBuf,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = usize> + Send>> {
                Box::pin(async move {
                    let mut cnt = 0;
                    if let Ok(mut entries) = fs::read_dir(&dir).await {
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

pub fn run_evolve_with(base_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let assessment = assessment_phase(base_dir)?;
    println!("[greatsage] Assessment Phase Result:\n{assessment}");
    Ok(())
}

pub fn run_evolve() -> Result<(), Box<dyn std::error::Error>> {
    run_evolve_with(Path::new("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assessment_phase_returns_nonempty() {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
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
        () = run_evolve_with(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .expect("run_evolve should complete without error");
    }
}
