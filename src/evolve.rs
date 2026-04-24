use std::fs;
use std::path::Path;
use std::time::Duration;
use tokio::time;

const TIMEOUT_SECS: u64 = 1200;

/// Perform the Assessment Phase (A1) of the evolve pipeline.
/// Collects basic self‑analysis data such as version, source file count,
/// and a placeholder CI status. The operation is wrapped in a timeout of
/// half the total evolve timeout.
pub fn assessment_phase() -> Result<String, Box<dyn std::error::Error>> {
    // Create a Tokio runtime to run the async timeout.
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        // Wrap the actual assessment work in a timeout future.
        let work = async {
            // Read Cargo.toml version.
            let cargo_toml = fs::read_to_string("Cargo.toml")?;
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
            fn count_rs(dir: &Path) -> usize {
                let mut cnt = 0;
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.filter_map(Result::ok) {
                        let path = entry.path();
                        if path.is_dir() {
                            cnt += count_rs(&path);
                        } else if path.extension().is_some_and(|e| e == "rs") {
                            cnt += 1;
                        }
                    }
                }
                cnt
            }
            let src_files = count_rs(Path::new("src"));

            // Placeholder for latest CI status.
            let ci_status = "unknown";

            Ok::<String, Box<dyn std::error::Error>>(format!(
                "Version: {version}\nSource files: {src_files}\nCI last: {ci_status}"
            ))
        };

        match time::timeout(Duration::from_secs(TIMEOUT_SECS / 2), work).await {
            Ok(res) => res,
            Err(_) => Err("assessment phase timed out".into()),
        }
    })
}

pub fn run_evolve() -> Result<(), Box<dyn std::error::Error>> {
    // Phase A1 – Assessment
    let assessment = assessment_phase()?;
    println!("[greatsage] Assessment Phase Result:\n{}", assessment);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assessment_phase_returns_nonempty() {
        let res = assessment_phase().expect("assessment_phase should succeed");
        assert!(
            !res.trim().is_empty(),
            "assessment result should not be empty"
        );
    }
}
