#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn test_strict_errors_panic_exit_code_101() {
        let output = Command::new("cargo")
            .args(["run", "--quiet", "--", "--strict-errors", "test.rs"])
            .env("FORCE_PANIC", "1")
            .env("ANTHROPIC_API_KEY", "dummy")
            .output()
            .expect("failed to execute cargo run");

        eprintln!("STDOUT: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR: {}", String::from_utf8_lossy(&output.stderr));

        assert_eq!(output.status.code(), Some(101));
    }
}
