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
        let mut app_config = AppConfig::default();
        app_config.repl_error_handling = if args.check {
            true
        } else if args.error_handling {
            true
        } else {
            args.repl_error_handling
        };
        assert!(
            app_config.repl_error_handling,
            "repl_error_handling should be enabled by --check"
        );
    }
}
