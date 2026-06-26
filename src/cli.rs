use {
    crate::{config::McpConfig, providers::Provider, repl::help_data::cli_repl_commands_section},
    bevy::ecs::resource::Resource,
    clap::{ArgAction, CommandFactory, Parser, Subcommand},
    clap_help::Printer,
    std::path::PathBuf,
    url::Url,
};

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    /// Interactive configuration wizard
    Setup,
}

#[derive(Clone, Debug, Parser, Resource)]
#[command(
    version,
    about,
    long_about = None,
    disable_help_flag = true,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Prompt file to execute
    #[arg(index = 1)]
    pub prompt_file: Option<PathBuf>,
    /// Model to use
    #[arg(long, value_name = "name", env = "MODEL")]
    pub model: Option<String>,
    /// Provider to use
    #[arg(long, value_name = "name", env = "PROVIDER")]
    pub provider: Option<Provider>,
    /// Custom API endpoint (e.g., http://localhost:11434/v1)
    #[arg(long, value_name = "url", env = "BASE_URL")]
    pub base_url: Option<Url>,
    /// Directory containing skill files (repeatable)
    #[arg(long, value_name = "dir", action = ArgAction::Append)]
    pub skills: Option<Vec<PathBuf>>,
    /// Custom system prompt (overrides default)
    #[arg(long, value_name = "text")]
    pub system: Option<String>,
    /// Read system prompt from file
    #[arg(long, value_name = "f")]
    pub system_file: Option<PathBuf>,
    /// Run a single prompt and exit (no REPL)
    #[arg(short, long, value_name = "t")]
    pub prompt: Option<String>,
    /// Write a final response text to a file
    #[arg(short, long, value_name = "f")]
    pub output: Option<PathBuf>,
    ///  API key (overrides provider-specific env var)
    #[arg(long, value_name = "key", env = "API_KEY")]
    pub api_key: Option<String>,
    /// MCP server to connect: HTTP URL or stdio command (repeatable)
    #[arg(long, value_name = "server", action = ArgAction::Append)]
    pub mcp: Option<Vec<McpConfig>>,
    /// Print the fully assembled system prompt and exit
    #[arg(long)]
    pub print_system_prompt: bool,
    /// Do not print startup hints, banner, and usage statistics
    #[arg(short = 'n', long)]
    pub no_hints: bool,
    /// Print help
    #[arg(short, long)]
    pub help: bool,
}

impl Cli {
    fn print_help() {
        let command = Cli::command().bin_name("greatsage");
        let mut printer = Printer::new(command);

        () = printer.set_template(
            "usage",
            "Usage: `greatsage [COMMAND] [PROMPT_FILE] [options]`",
        );

        () = printer.template_keys_mut().push("subcommands");
        () = printer.set_template(
            "subcommands",
            r#"
**Subcommands:**
  setup    Interactive configuration wizard
"#,
        );

        () = printer.template_keys_mut().push("repl-commands");
        let repl_commands = cli_repl_commands_section();
        () = printer.set_template("repl-commands", &repl_commands);

        () = printer.template_keys_mut().push("environment");
        () = printer.set_template(
            "environment",
            r#"
**Environment:**
  PROVIDER            Provider to use (via env or `--provider` flag)
  API_KEY             Fallback API key (any provider)

  ANTHROPIC_API_KEY   API key for Anthropic (default provider)
  CEREBRAS_API_KEY    API key for Cerebras
  DEEPSEEK_API_KEY    API key for DeepSeek
  GOOGLE_API_KEY      API key for Google/Gemini
  GROQ_API_KEY        API key for Groq
  MISTRAL_API_KEY     API key for Mistral
  MINIMAX_API_KEY     API key for MiniMax
  OPENAI_API_KEY      API key for OpenAI
  OPENROUTER_API_KEY  API key for OpenRouter
  XAI_API_KEY         API key for xAI
  ZAI_API_KEY         API key for ZAI (Zhipu AI / z.ai)
  BASE_URL            Custom base URL (mainly used with `--provider` custom)

**Config files (searched in order, first found wins):**
  .greatsage/config.toml           Project-level config (current directory)
  ~/.config/greatsage/config.toml  User-level config (XDG)

**API keys in config.toml use `env!VAR` references:**
  anthropic_api_key = "env!ANTHROPIC_API_KEY"

**Environment files (merged low → high at startup; shell env wins):**
  ~/.config/greatsage/.env
  .greatsage/.env            (walk-up from cwd)
  ./.env                     (walk-up from cwd)

  Paired `.env` beside `config.toml` also resolves `env!VAR` when not already in the shell.
"#,
        );

        let skin = printer.skin_mut();
        skin.table_border_chars = termimad::ROUNDED_TABLE_BORDER_CHARS;

        () = printer.print_help();
    }

    pub fn parse_and_check_help() -> Self {
        let cli = Self::parse();
        if cli.help {
            () = Self::print_help();
            std::process::exit(0);
        }
        cli
    }
}
