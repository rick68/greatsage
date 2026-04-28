#[cfg(test)]
mod tests {
    use {crate::evolve, std::fs, tempfile::TempDir};

    #[test]
    fn test_assessment_phase_includes_repl_status() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // 1. Initially disabled (no config.toml)
        let res = evolve::assessment_phase(base).unwrap();
        assert!(res.contains("REPL Error Handling: disabled"));

        // 2. Enabled after creating config.toml with flag true
        let config_path = base.join("config.toml");
        () = fs::write(config_path, "repl_error_handling = true").unwrap();
        let res = evolve::assessment_phase(base).unwrap();
        assert!(res.contains("REPL Error Handling: enabled"));
    }
}
