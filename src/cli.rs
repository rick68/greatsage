use {
    crate::providers::Provider,
    bevy::ecs::resource::Resource,
    clap::{self, Parser, Subcommand, ValueEnum},
    std::path::PathBuf,
};

/// Context management strategy.
#[derive(Clone, Copy, Debug, Default, PartialEq, ValueEnum)]
pub enum ContextStrategy {
    /// Default: auto-compact conversation when approaching context limit
    #[default]
    Compaction,
    /// Write checkpoint file and exit with code 2 when approaching limit
    Checkpoint,
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Parser, Resource)]
#[command(
    version = VERSION,
    about,
    long_about = None,
    disable_help_subcommand = true,
    disable_help_flag = true
)]
pub struct Cli {
    /// Model to use
    #[arg(long, value_name = "name")]
    pub model: Option<String>,
    /// Choose the LLM provider
    #[arg(long, value_name = "name")]
    pub provider: Option<Provider>,
    /// Run a single prompt and exit (no REPL)
    #[arg(short, long, value_name = "t")]
    pub prompt: Option<String>,
    /// Directory containing skill files
    #[arg(long, value_name = "dir")]
    pub skills: Option<Vec<PathBuf>>,
    /// Optional configuration file path (default: $HOME/.greatsage.toml)
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
    /// Context management: compaction or checkpoint
    #[arg(long, value_name = "s", default_value = "compaction")]
    pub context_strategy: ContextStrategy,
    /// Subcommands
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Print help
    #[arg(short = 'h', long, help = "Print help information")]
    pub help: bool,
}

/// CLI subcommands
#[derive(Subcommand, Clone, Debug, PartialEq)]
/// Available subcommands for the greatsage CLI.
pub enum Command {
    /// Show help information
    Help,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::cli::{Cli, ContextStrategy},
        crate::providers::Provider,
        clap::error::ErrorKind,
        pretty_assertions::assert_eq,
    };

    #[test]
    fn defaults_when_no_args() {
        // Simulate calling the binary with just its name
        let cli = Cli::try_parse_from(["greatsage"]).expect("should parse defaults");
        assert_eq!(cli.model, None);
        assert_eq!(cli.context_strategy, ContextStrategy::Compaction);
    }

    #[test]
    fn version_flag_triggers_display() {
        let result = Cli::try_parse_from(["greatsage", "--version"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Clap returns DisplayVersion when --version is used
        assert_eq!(err.kind(), ErrorKind::DisplayVersion);
    }

    #[test]
    fn provider_flag_parses() {
        let cli = Cli::try_parse_from(["greatsage", "--provider", "custom"])
            .expect("parse provider flag");
        assert_eq!(cli.provider, Some(Provider::Custom));
        // other defaults remain unchanged
        assert_eq!(cli.model, None);
        assert_eq!(cli.context_strategy, ContextStrategy::Compaction);
    }

    #[test]
    fn default_provider_when_omitted() {
        // No provider flag supplied
        let cli = Cli::try_parse_from(["greatsage"]).expect("should parse defaults");
        // Direct cli.provider should be None
        assert_eq!(cli.provider, None);
        // After applying default logic, should be Anthropic
        let provider = cli.provider.unwrap_or(Provider::Anthropic);
        assert_eq!(provider, Provider::Anthropic);
    }
}
