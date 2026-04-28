#[cfg(test)]
mod tests {
    use {
        std::{fs, process::Command},
        tempfile::Builder,
    };

    #[test]
    fn test_check_flag_persists_to_config() {
        // Create a temporary config file.
        let tmp_file = Builder::new()
            .suffix(".toml")
            .tempfile()
            .expect("failed to create temp config file");
        let config_path = tmp_file.path().to_path_buf();
        // Ensure the file exists (load_or_create will create if missing).
        // Run the binary with --check and the custom config path.
        // Use `cargo run` to build and execute the binary.
        let output = Command::new("cargo")
            .args([
                "run",
                "--quiet",
                "--",
                "--check",
                "--config",
                &config_path.to_string_lossy(),
            ])
            .output()
            .expect("failed to execute cargo run");
        assert!(
            output.status.success(),
            "binary exited with failure: {:?}",
            output
        );
        // Read the config file and verify repl_error_handling is true.
        let contents = fs::read_to_string(&config_path).expect("failed to read config file");
        assert!(
            contents.contains("repl_error_handling = true"),
            "config did not contain persisted flag: {}",
            contents
        );
    }
}
