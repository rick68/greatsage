use {
    crate::{
        auth::provider_cli_help_block, config::McpConfig, providers::Provider,
        repl::help_data::cli_repl_commands_section,
    },
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
    /// Full-screen TUI (Grok-aligned scrollback + prompt; Session ECS truth)
    Tui,
    /// Manage credentials (login, logout, status, list). See `greatsage auth --help`.
    Auth {
        #[command(subcommand)]
        action: AuthCommand,
    },
    /// OAuth login (alias for `auth login`). See `greatsage login --help`.
    Login {
        /// Provider id (default: config provider, else `xai`). Only `xai` has OAuth today.
        #[arg(value_name = "PROVIDER")]
        provider: Option<String>,
        /// Prefer OAuth adapter path (default for oauth-capable providers)
        #[arg(long)]
        oauth: bool,
        /// Use the device-code OAuth grant. Alias: --device-auth
        #[arg(long, alias = "device-auth")]
        device_code: bool,
        /// Authorization-code + loopback PKCE instead of device code
        #[arg(long)]
        authorization_code: bool,
        /// Print URL/code only; do not open a browser
        #[arg(long)]
        no_browser: bool,
        /// Replace existing OAuth tokens
        #[arg(long)]
        force: bool,
    },
    /// Clear stored OAuth tokens (alias for `auth logout`)
    Logout {
        /// Provider id (default: config provider, else `xai`)
        #[arg(value_name = "PROVIDER")]
        provider: Option<String>,
    },
}

#[derive(Clone, Debug, Subcommand)]
pub enum AuthCommand {
    /// OAuth 2.1 + PKCE login (oauth-capable providers; use --device-code)
    Login {
        #[arg(value_name = "PROVIDER")]
        provider: Option<String>,
        #[arg(long)]
        oauth: bool,
        #[arg(long, alias = "device-auth")]
        device_code: bool,
        #[arg(long)]
        authorization_code: bool,
        #[arg(long)]
        no_browser: bool,
        #[arg(long)]
        force: bool,
    },
    /// Delete stored OAuth tokens for a provider
    Logout {
        #[arg(value_name = "PROVIDER")]
        provider: Option<String>,
    },
    /// OAuth login status (active provider, or all with PROVIDER / see auth list)
    Status {
        #[arg(value_name = "PROVIDER")]
        provider: Option<String>,
    },
    /// List every catalog provider × key / oauth / mode
    List,
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
    /// Resume last auto-saved REPL session from `.greatsage/last-session.json` in cwd
    #[arg(short = 'c', long = "continue")]
    pub continue_session: bool,
    /// Minimal mode (Claude Code parity): skip auto-loaded project context, memories,
    /// skills, and MCP; use explicit --skills / --mcp to opt back in
    #[arg(short = 'b', long)]
    pub bare: bool,
    /// Do not print startup hints, banner, and usage statistics
    #[arg(short = 'n', long)]
    pub no_hints: bool,
    /// Headless BRP / ECS server (no REPL). Requires `dev_native`, non-TTY ok.
    ///
    /// Without this flag, a non-interactive stdin and no `-p` prompt exits immediately —
    /// accidental `greatsage -n` in the background must not run forever.
    #[arg(long)]
    pub headless: bool,
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
  setup                 Interactive configuration wizard
  tui                   Full-screen TUI (scrollback + prompt; Session ECS)
  auth                  Credentials: login | logout | status | list
  login [PROVIDER]      OAuth login (alias: auth login; see `login --help`)
  logout [PROVIDER]     Clear OAuth tokens (alias: auth logout)

**Interactive entry:**
  greatsage             Line-oriented REPL (default on a TTY)
  greatsage tui         Full-screen TUI
  greatsage -p TEXT     One-shot prompt (no interactive UI)
"#,
        );

        () = printer.template_keys_mut().push("repl-commands");
        let repl_commands = cli_repl_commands_section();
        () = printer.set_template("repl-commands", &repl_commands);

        () = printer.template_keys_mut().push("environment");
        let oauth_capable = crate::auth::providers::oauth_capable_provider_ids().join(", ");
        // Tab-align descriptions (real `\t`; raw strings would keep the two chars `\t`).
        let oauth_cmds = [
            ("login --help", "PROVIDER table + flags"),
            ("login <provider> --device-code", "Device-code grant"),
            (
                "login <provider> --device-code --no-browser",
                "Print URL only (SSH)",
            ),
            ("auth status", "Active OAuth login (if any)"),
            ("auth status [provider]", "Detail for one provider"),
            ("auth list", "All providers x credentials"),
            ("logout [provider]", "Clear stored tokens"),
        ]
        .map(|(cmd, desc)| help_tab_row(cmd, desc))
        .join("\n");
        let environment = format!(
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

**API keys in config.toml use env!VAR references (example):**
  anthropic_api_key = "env!ANTHROPIC_API_KEY"

**OAuth 2.1 + PKCE:**
  Account login for oauth-capable providers (see login --help for the table).
  Static API keys still work for every provider; mode = auto prefers a key
  when set. Tokens go under ~/.config/greatsage/tokens/<provider>.json.

  Currently oauth-capable: {oauth_capable}

{oauth_cmds}

  Config (optional, in config.toml):
  [auth.<provider>]
  mode  = "auto"           # auto / api_key / oauth
  grant = "device_code"    # device_code / authorization_code

**Environment files (merged low → high at startup; shell env wins):**
  ~/.config/greatsage/.env
  .greatsage/.env            (walk-up from cwd)
  ./.env                     (walk-up from cwd)

  Paired .env beside config.toml also resolves env!VAR when not already in the shell.
"#,
            oauth_capable = oauth_capable,
            oauth_cmds = oauth_cmds,
        );
        () = printer.set_template("environment", &environment);

        let skin = printer.skin_mut();
        skin.table_border_chars = termimad::ROUNDED_TABLE_BORDER_CHARS;

        () = printer.print_help();
    }

    pub fn parse_and_check_help() -> Self {
        // Top-level `disable_help_flag` (custom clap-help printer) is inherited by
        // subcommands, so `greatsage login --help` would otherwise be "unexpected".
        // Intercept before parse and print the subcommand's long help.
        if let Some(sub) = Self::subcommand_help_request() {
            Self::print_subcommand_help(sub);
            std::process::exit(0);
        }

        let cli = Self::parse();
        if cli.help {
            () = Self::print_help();
            std::process::exit(0);
        }
        cli
    }

    /// Known top-level subcommand names (keep in sync with [`Command`]).
    const SUBCOMMANDS: &'static [&'static str] = &["setup", "tui", "auth", "login", "logout"];

    /// If argv is `<sub> … --help|-h …` (or `--help` before `<sub>`), return that sub name.
    fn subcommand_help_request() -> Option<&'static str> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let wants_help = args.iter().any(|a| a == "--help" || a == "-h");
        if !wants_help {
            return None;
        }
        // Prefer more specific auth subcommands: `auth login --help` → login help
        if args.iter().any(|a| a == "auth") {
            if args.iter().any(|a| a == "login") {
                return Some("login");
            }
            if args.iter().any(|a| a == "logout") {
                return Some("logout");
            }
            if args.iter().any(|a| a == "status") {
                return Some("auth-status");
            }
            if args.iter().any(|a| a == "list") {
                return Some("auth-list");
            }
            return Some("auth");
        }
        for arg in &args {
            if let Some(sub) = Self::SUBCOMMANDS
                .iter()
                .copied()
                .find(|s| *s == arg.as_str())
            {
                return Some(sub);
            }
        }
        None
    }

    fn print_subcommand_help(sub: &str) {
        match sub {
            "login" => print!("{}", login_help_text()),
            "logout" => print!("{}", logout_help_text()),
            "auth" => print!("{}", auth_help_text()),
            "auth-status" => print!("{}", auth_status_help_text()),
            "auth-list" => print!("{}", auth_list_help_text()),
            other => {
                let mut cmd = Cli::command().bin_name("greatsage");
                match cmd.find_subcommand_mut(other) {
                    Some(sub_cmd) => {
                        let _ = sub_cmd.print_long_help();
                        println!();
                    }
                    None => Self::print_help(),
                }
            }
        }
    }
}

/// `  {left}<tabs>{right}` so descriptions share a column under common tab stops (8).
fn help_tab_row(left: &str, right: &str) -> String {
    const TAB: u8 = 8;
    /// First description column (must clear the longest command left-hand side).
    const DESC_COL: usize = 48;
    let mut line = format!("  {left}");
    let mut col = line.chars().count();
    if col >= DESC_COL {
        line.push('\t');
    } else {
        while col < DESC_COL {
            line.push('\t');
            col = (col / usize::from(TAB) + 1) * usize::from(TAB);
        }
    }
    line.push_str(right);
    line
}

/// Operator-facing recipe; PROVIDER table is generated so ids cannot drift.
fn login_help_text() -> String {
    let catalog = provider_cli_help_block();
    let oauth_capable = crate::auth::providers::oauth_capable_provider_ids().join(", ");
    format!(
        "\
greatsage login — OAuth 2.1 + PKCE (alias: auth login)

USAGE
  greatsage login [PROVIDER] [OPTIONS]
  greatsage auth login [PROVIDER] [OPTIONS]

  PROVIDER omitted → config provider, else xai (fallback id only).
  Must be an id from the table (same as --provider).
  Only oauth-capable ids accept login (see table; today: {oauth_capable}).

OPTIONS
  --device-code, --device-auth  Device-code OAuth grant
  --no-browser                  Print URL + user code only (SSH/remote)
  --authorization-code          Loopback auth-code + PKCE instead
  --oauth                       Prefer OAuth path (documented intent)
  --force                       Replace existing tokens

{catalog}
CONFIG (non-secret; config.toml)

  [auth]
  default_mode = \"auto\"       # auto / api_key / oauth

  [auth.<provider>]           # e.g. [auth.xai]
  mode  = \"oauth\"             # auto / api_key / oauth
  grant = \"device_code\"       # device_code / authorization_code

  # Optional non-secret overrides (same table):
  #   client_id
  #   authorization_endpoint
  #   token_endpoint
  #   device_authorization_endpoint
  #   scopes

WHAT IT DOES (--device-code)
  1. Requests a device code from the provider authorization server (PKCE S256)
  2. Prints verification URL + user code (optional open browser)
  3. Polls until approved; stores tokens under
     ~/.config/greatsage/tokens/<provider>.json
  4. Never prints access/refresh tokens

WHEN TO USE LOGIN VS API KEY
  • Have a provider API key → skip login (mode auto: key wins).
  • Want account OAuth → login <provider> --device-code (oauth-capable only).
  • Key-only providers → set env / env!VAR; login is rejected.

RECIPE (OAuth)
  greatsage login <provider> --device-code
  # SSH: greatsage login <provider> --device-code --no-browser
  unset the provider API key env if mode=auto would prefer the key
  greatsage -p \"hello\" -n
  greatsage logout <provider>

RECIPE (API key)
  export <PROVIDER>_API_KEY=\"…\"   # see table, e.g. XAI_API_KEY
  export PROVIDER=<provider>
  greatsage -p \"hello\" -n

SEE ALSO
  greatsage auth status [provider]
  greatsage auth list
  greatsage setup
  Code: providers/mod.rs PROVIDER_SPECS · auth/providers/ is_oauth_capable
"
    )
}

fn logout_help_text() -> String {
    let catalog = provider_cli_help_block();
    format!(
        "\
greatsage logout — delete stored OAuth tokens (alias: auth logout)

USAGE
  greatsage logout [PROVIDER]
  greatsage auth logout [PROVIDER]

  PROVIDER defaults to config provider, else **xai**.

{catalog}
WHAT IT DOES
  Removes ~/.config/greatsage/tokens/<provider>.json if present.
  Does NOT touch env vars or API keys in config.toml / .env.

EXAMPLES
  greatsage logout xai
  greatsage logout

SEE ALSO
  greatsage login --help
  greatsage auth status
"
    )
}

fn auth_help_text() -> String {
    format!(
        "\
greatsage auth — credential management

USAGE
  greatsage auth <COMMAND>

COMMANDS
  login [PROVIDER] [OPTIONS]   OAuth 2.1 + PKCE (see: greatsage login --help)
  logout [PROVIDER]            Clear token store entry
  status [PROVIDER]            Non-secret status (mode, key, oauth, effective)
  list                         Table of all catalog providers

This build: OAuth adapter for **xai** only. Other providers use API keys.

SEE ALSO
  greatsage login --help
"
    )
}

fn auth_status_help_text() -> String {
    "\
greatsage auth status — non-secret OAuth / credential snapshot

USAGE
  greatsage auth status              Active OAuth+PKCE login (if any)
  greatsage auth status [PROVIDER]   Detail for that provider only

  No PROVIDER → if a provider has stored OAuth tokens, print its multi-line
  status (prefer config provider when it has tokens). If none, report that
  there is no OAuth+PKCE login (not a full catalog).
  With PROVIDER → multi-line snapshot for that id (never tokens).
  Full catalog table: greatsage auth list
"
    .to_owned()
}

fn auth_list_help_text() -> String {
    "\
greatsage auth list — catalog overview

USAGE
  greatsage auth list

  One row per PROVIDER_SPECS id: mode, key yes/no, oauth yes/no, effective.
  Differs from bare `auth status` (that shows only active OAuth login detail).
"
    .to_owned()
}
