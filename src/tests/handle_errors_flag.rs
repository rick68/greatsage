#[cfg(test)]
mod tests {
    use crate::cli::Args;
    use clap::Parser;
    use crate::config::AppConfig;

    #[test]
    fn test_handle_errors_flag_enables_repl_error_handling() {
        let args = Args::parse_from(["test_bin", "--handle-errors"]);
        assert!(args.handle_errors, "--handle-errors should be true");
        let app_config = AppConfig {
            repl_error_handling: args.check || args.error_handling || args.handle_errors || args.repl_error_handling,
            ..Default::default()
        };
        assert!(app_config.repl_error_handling, "repl_error_handling should be enabled by --handle-errors");
    }
}
