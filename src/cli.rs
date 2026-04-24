use {
    crate::config::{ConfigSubcommand, ContextStrategy, default_config_path},
    clap::{
        ArgAction, CommandFactory, Parser, Subcommand,
        builder::styling::{AnsiColor, Effects, Styles},
    },
    std::path::PathBuf,
};

pub const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .invalid(AnsiColor::Yellow.on_default().effects(Effects::BOLD));

#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None, styles = STYLES)]
pub struct Args {
    /// Path to config file
    #[arg(long, value_name = "PATH", default_value_os_t = default_config_path())]
    pub config: PathBuf,
    /// Model to use (overrides config file)
    #[arg(long, value_name = "name")]
    pub model: Option<String>,
    /// Run a single prompt and exit (no REPL)
    #[arg(short, long, value_name = "t")]
    pub prompt: Option<String>,
    /// Positional prompt argument (alternative to --prompt)
    #[arg(value_name = "prompt", required = false)]
    pub positional_prompt: Option<String>,
    /// Directory containing skill files
    #[arg(long, value_name = "dir", action = ArgAction::Append)]
    pub skills: Vec<PathBuf>,
    /// MCP server to connect: HTTP URL or stdio command. Repeatable.
    #[arg(long, value_name = "server", action = ArgAction::Append)]
    pub mcp: Vec<String>,
    /// Context management: compaction or checkpoint (overrides config file)
    #[arg(long, value_name = "s")]
    pub context_strategy: Option<ContextStrategy>,
    /// Print status messages to stderr in non-interactive mode
    #[arg(short = 'v', long)]
    pub verbose: bool,
    /// Stage all changes before running the app
    #[arg(long, action = ArgAction::SetTrue)]
    pub stage_all: bool,
    /// Commit staged changes with the given message after optional staging
    #[arg(long, value_name = "msg")]
    pub git_commit: Option<String>,
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Run evolve mode (placeholder)
    #[arg(long, action = ArgAction::SetTrue)]
    pub evolve: bool,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    /// View and edit configuration
    Config {
        #[command(subcommand)]
        cmd: ConfigSubcommand,
    },
}

pub fn complete() {
    () = clap_complete::CompleteEnv::with_factory(Args::command).complete();
}
