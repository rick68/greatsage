use {
    bevy::ecs::resource::Resource,
    clap::{CommandFactory, Parser},
    clap_help::Printer,
    std::path::PathBuf,
};

#[derive(Clone, Debug, Parser, Resource)]
#[command(
    version,
    about,
    long_about = None,
    disable_help_flag = true,
)]
pub struct Cli {
    /// Model to use
    #[arg(
        long,
        value_name = "name",
        default_value = "claude-opus-4-7",
        env = "MODEL"
    )]
    pub model: Option<String>,
    /// Custom API endpoint (e.g., http://localhost:11434/v1)
    #[arg(long, value_name = "url", env = "BASE_URL")]
    pub base_url: Option<String>,
    /// Directory containing skill files
    #[arg(long, value_name = "dir")]
    pub skills: Option<Vec<PathBuf>>,
    /// Run a single prompt and exit (no REPL)
    #[arg(short, long, value_name = "t")]
    pub prompt: Option<String>,
    ///  API key (overrides provider-specific env var)
    #[arg(long, value_name = "key", env = "API_KEY")]
    pub api_key: Option<String>,
    /// Print help
    #[arg(long)]
    pub help: bool,
}

impl Cli {
    fn print_help() {
        let mut printer = Printer::new(Cli::command());

        printer.template_keys_mut().push("repl-commands");
        printer.set_template(
            "repl-commands",
            r#"
**Commands (in REPL):**
  /quit, /exit     Exit the agent
  /clear           Clear conversation history
  /model <name>    Switch model mid-session
"#,
        );

        printer.template_keys_mut().push("environment");
        printer.set_template(
            "environment",
            r#"
**Environment:**
  ANTHROPIC_API_KEY    API key for Anthropic (required)
  API_KEY              Alternative env var for API key
"#,
        );

        let skin = printer.skin_mut();
        skin.table_border_chars = termimad::ROUNDED_TABLE_BORDER_CHARS;

        printer.print_help();
    }

    pub fn parse_and_check_help() -> Self {
        let cli = Self::parse();
        if cli.help {
            Self::print_help();
            std::process::exit(0);
        }
        cli
    }
}
