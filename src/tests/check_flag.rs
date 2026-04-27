#[cfg(test)]
mod tests {
    use {
        crate::{cli::Args, config::AppConfig},
        clap::Parser,
    };

    #[test]
    fn test_check_flag_overrides_other_flags() {
        // Simulate args parsing with --check flag only.
        let args = Args::parse_from(["test_bin", "--check"]);
        assert!(args.check, "--check flag should be true");
        // Simulate the flag processing as in main.rs.
        let app_config = AppConfig {
            repl_error_handling: args.check || args.error_handling || args.repl_error_handling,
            ..Default::default()
        };
        assert!(
            app_config.repl_error_handling,
            "repl_error_handling should be enabled by --check"
        );
    }
}
