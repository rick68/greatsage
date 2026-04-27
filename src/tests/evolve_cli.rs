#[cfg(test)]
mod tests {
    use {crate::cli::{Args, Command}, clap::Parser};

    #[test]
    fn test_args_parsing_evolve_subcommand() {
        let args = Args::parse_from(["test_bin", "evolve"]);
        match args.command {
            Some(Command::Evolve { dry_run }) => {
                // Ensure dry_run defaults to false
                assert!(!dry_run, "dry_run flag should be false by default");
            }
            other => panic!("Expected Command::Evolve, got {other:?}"),
        }
        // Ensure backward compatibility flag is false
        assert!(!args.evolve, "evolve flag should be false when using subcommand");
    }
}
