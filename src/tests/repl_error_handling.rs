#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use {
        crate::handle_prompt, crate::maybe_set_strict_error_hook, std::io::Write, tempfile::Builder,
    };

    #[test]
    fn test_flag_disabled_accepts_any_prompt() {
        let res = handle_prompt("any input".to_string(), false);
        assert!(res.is_ok(), "Flag disabled should accept any prompt");
    }

    #[test]
    fn test_flag_enabled_existing_file() {
        // Create a temporary .txt file.
        let mut tmp = Builder::new().suffix(".txt").tempfile().expect("temp file");
        writeln!(tmp, "temporary content").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let res = handle_prompt(path.clone(), true);
        assert!(res.is_ok(), "Existing file should be accepted: {path}");
    }

    #[test]
    fn test_forced_panic_is_caught() {
        // Set environment variable to trigger panic inside handle_prompt.
        unsafe {
            std::env::set_var("FORCE_PANIC", "1");
        }

        // Since REPL error handling flag must be true to enable the panic simulation.
        let result = std::panic::catch_unwind(|| {
            let _ = handle_prompt("any input".to_string(), true);
        });
        // The panic should be caught and not unwind the test.
        assert!(result.is_err(), "Expected panic to be triggered and caught");
        // Clean up env var.
        unsafe {
            std::env::remove_var("FORCE_PANIC");
        }
    }
}
