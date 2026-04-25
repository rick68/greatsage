#[cfg(test)]
mod tests {
    use crate::handle_prompt;
    use tempfile::Builder;
    use std::io::Write;

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
        assert!(res.is_ok(), "Existing file should be accepted: {}", path);
    }

    #[test]
    fn test_flag_enabled_nonexistent_file() {
        let path = "nonexistent_file.rs".to_string();
        let res = handle_prompt(path.clone(), true);
        assert!(res.is_err(), "Nonexistent file should produce error");
    }
}
