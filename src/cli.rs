use {
    bevy::ecs::resource::Resource,
    clap::{Parser, ValueEnum},
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

#[derive(Clone, Debug, Parser, Resource)]
#[command(version, about, long_about = None)]
pub struct Cli {
    // Model to use
    #[arg(long, value_name = "name", default_value = "claude-opus-4-7")]
    pub model: Option<String>,
    /// Run a single prompt and exit (no REPL)
    #[arg(short, long, value_name = "t")]
    pub prompt: Option<String>,
    /// Directory containing skill files
    #[arg(long, value_name = "dir")]
    pub skills: Option<Vec<PathBuf>>,
    /// Context management: compaction or checkpoint
    #[arg(long, value_name = "s", default_value = "compaction")]
    pub context_strategy: ContextStrategy,
}
