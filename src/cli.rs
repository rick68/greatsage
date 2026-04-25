use {
    crate::config::{ConfigSubcommand, ContextStrategy, ThinkingLevel, default_config_path},
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

fn repl_help() -> String {
    let h = STYLES.get_header();
    let l = STYLES.get_literal();
    let (hr, lr) = (h.render(), l.render());
    let (hx, lx) = (h.render_reset(), l.render_reset());
    format!(
        "\
{hr}REPL Commands:{hx}

  {hr}Session:{hx}
    {lr}/help{lx}              Show this help
    {lr}/clear{lx}             Clear output
    {lr}/quit{lx}, {lr}/exit{lx}       Exit greatsage

  {hr}Git:{hx}
    {lr}/git stage{lx}         Stage all changes
    {lr}/git commit -m …{lx}   Commit staged changes
    {lr}/git revert{lx}        Revert last commit"
    )
}

#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None, styles = STYLES, after_help = repl_help(), term_width = 0)]
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
    /// Enable extended thinking (off, minimal, low, medium, high)
    #[arg(long, value_name = "lvl")]
    pub thinking: Option<ThinkingLevel>,
    /// Maximum output tokens per response
    #[arg(long, value_name = "n")]
    pub max_tokens: Option<u32>,
    /// Maximum agent turns per prompt
    #[arg(long, value_name = "n")]
    pub max_turns: Option<usize>,
    /// Sampling temperature (0.0-1.0)
    #[arg(long, value_name = "f")]
    pub temperature: Option<f32>,
    /// Run a subcommand (e.g., config)
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Run evolve mode (placeholder)
    #[arg(long, action = ArgAction::SetTrue)]
    pub evolve: bool,
    /// Enable verbose output
    #[arg(short = 'v', long, action = ArgAction::SetTrue)]
    pub verbose: bool,
    /// Enable REPL error handling validation
    #[arg(long, action = ArgAction::SetTrue)]
    pub error_handling: bool,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    /// View and edit configuration
    Config {
        #[command(subcommand)]
        cmd: ConfigSubcommand,
    },
    /// Display basic project statistics (version, source files, CI status)
    #[command(about = "Display basic project statistics (version, source files, CI status)")]
    Stats,
    /// Run the self‑evolution pipeline
    #[command(about = "Run the self‑evolution pipeline")]
    Evolve,
}

pub fn complete() {
    () = clap_complete::CompleteEnv::with_factory(Args::command).complete();
}
