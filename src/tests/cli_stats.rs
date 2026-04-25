// Test for the new `stats` subcommand

#[cfg(test)]
mod tests {
    use {
        crate::{
            cli::{Args, Command},
            evolve,
        },
        clap::Parser,
        std::path::Path,
    };

    #[test]
    fn test_cli_stats_subcommand_outputs_version() {
        // Build Args as if called with `greatsage stats`
        let args = Args::parse_from(["greatsage", "stats"]);
        // Ensure the command is parsed as Stats
        match args.command {
            Some(Command::Stats) => {}
            other => panic!("Expected Command::Stats, got {other:?}"),
        }
        // Call the assessment phase directly (the subcommand would invoke this)
        let info =
            evolve::assessment_phase(Path::new(".")).expect("assessment_phase should succeed");
        // The result should contain a version line
        assert!(
            info.contains("Version:"),
            "Assessment info missing Version line: {info}"
        );
        // Also should contain Source files line
        assert!(
            info.contains("Source files:"),
            "Assessment info missing Source files line: {info}"
        );
    }
}
