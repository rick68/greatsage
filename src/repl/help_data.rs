//! Slash-command metadata: registry, `/help` text, and inline arg hints.
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.
//! [`KNOWN_COMMANDS`] is the canonical registry and stays near the top.

const QUIT_EXIT_DETAIL: &str = concat!(
    "Aliases: /quit, /exit\n",
    "\n",
    "Exits the interactive REPL. Unsaved session data will be lost\n",
    "unless you /save first.\n"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplCommandCategory {
    Session,
    Git,
    Project,
    Ai,
}

impl ReplCommandCategory {
    const ALL: &[Self] = &[Self::Session, Self::Git, Self::Project, Self::Ai];

    const fn title(self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::Git => "Git",
            Self::Project => "Project",
            Self::Ai => "AI",
        }
    }

    fn section_header(self) -> String {
        format!("── {} ──", self.title())
    }
}

/// Which command list is being rendered (`/help` vs CLI `--help`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HelpListAudience {
    /// REPL `/help` grouped list (shorter primary blurbs).
    Repl,
    /// CLI `greatsage --help` REPL section (longer primary blurbs when set).
    Cli,
}

/// Presentation-only extra usage line under a command in help lists.
///
/// Not a separate [`ReplCommand`] / route — intentional multi-line usage
/// (e.g. yoyo `/history` + `/history detail`). No per-audience blurb: extras
/// have no `/help <cmd>` identity; only the parent command does.
#[derive(Debug, Clone, Copy)]
pub struct HelpUsageExtraLine {
    /// Left column, e.g. `/history detail`.
    pub label: &'static str,
    /// Right column (same for `/help` and CLI `--help` lists).
    pub summary: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct ReplCommand {
    pub name: &'static str,
    /// Right-column blurb for REPL `/help` list.
    pub summary: &'static str,
    /// Optional blurb for CLI `--help` list; [`None`] uses [`Self::summary`].
    pub cli_summary: Option<&'static str>,
    /// Inline ghost hint override; [`None`] uses [`Self::summary`].
    pub short_description: Option<&'static str>,
    pub category: ReplCommandCategory,
    /// Argument synopsis in primary list label (`[opt]`, `<required>`, or empty).
    pub args: &'static str,
    /// Inline ghost hint after `cmd `; uses [`Self::args`] when empty.
    pub arg_hint: &'static str,
    /// Extra usage lines after the primary entry (presentation only).
    pub help_extra_lines: &'static [HelpUsageExtraLine],
    pub usage: &'static str,
    /// Explanatory body for `/help <cmd>` and error hints; does not repeat [`Self::usage`].
    /// Prefer [`concat!`] for multi-line text so Rust source indent does not leak into output.
    pub detail: &'static str,
}

pub const KNOWN_COMMANDS: &[ReplCommand] = &[
    ReplCommand {
        name: "/help",
        summary: "Show this help",
        cli_summary: None,
        short_description: Some("Show help for commands"),
        category: ReplCommandCategory::Session,
        args: "[command]",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/help [command] - Show help information",
        detail: concat!(
            "Usage:\n",
            "  /help\t\t\tShow all available commands\n",
            "  /help <command>\tShow detailed help for a specific command\n",
            "\n",
            "Examples:\n",
            "  /help\n",
            "  /help add\n",
            "  /help commit\n",
        ),
    },
    ReplCommand {
        name: "/quit",
        summary: "Exit greatsage",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/quit  (alias: /exit)",
        detail: QUIT_EXIT_DETAIL,
    },
    ReplCommand {
        name: "/exit",
        summary: "Exit greatsage (alias for /quit)",
        cli_summary: None,
        short_description: Some("Exit greatsage"),
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/exit  (alias: /quit)",
        detail: QUIT_EXIT_DETAIL,
    },
    ReplCommand {
        name: "/clear",
        summary: "Clear conversation history",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/clear — Clear conversation history",
        detail: concat!(
            "Resets the conversation to a fresh state, removing all messages.\n",
            "If the conversation has more than 4 messages, asks for confirmation.\n",
            "The system prompt and loaded context are preserved.\n",
            "Session cost tracking continues.\n",
            "\n",
            "See also: /clear! (skip confirmation)\n"
        ),
    },
    ReplCommand {
        name: "/clear!",
        summary: "Force-clear without confirmation",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/clear! — Force-clear conversation history",
        detail: concat!(
            "Same as /clear but skips the confirmation prompt.\n",
            "Always clears immediately regardless of message count.\n",
        ),
    },
    ReplCommand {
        name: "/compact",
        summary: "Compact conversation to save context",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "[N|all|--preview]",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/compact [N|all|--preview] — Compact conversation to save context space",
        detail: concat!(
            "Usage:\n",
            "  /compact\t\tDefault compaction\n",
            "  /compact N\t\tKeep the last N messages at full fidelity\n",
            "  /compact all\t\tAggressive compaction (keep_recent = 2)\n",
            "  /compact --preview\tDry-run: show estimated before/after counts\n",
            "\n",
            "Summarizes older conversation into a shorter representation,\n",
            "freeing context window space on long sessions.\n"
        ),
    },
    ReplCommand {
        name: "/save",
        summary: "Save session to file",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "[path]",
        arg_hint: "<filename.json>",
        help_extra_lines: &[],
        usage: "/save [path] — Save session to file",
        detail: concat!(
            "Usage:\n",
            "  /save\t\t\tSave to yoyo-session.json\n",
            "  /save <path>\t\tSave to specified path\n",
            "\n",
            "Saves the full conversation history to a JSON file so it can\n",
            "be resumed later with /load.\n",
            "\n",
            "Examples:\n",
            "  /save\n",
            "  /save my-debug-session.json\n",
        ),
    },
    ReplCommand {
        name: "/load",
        summary: "Load session from file",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "[path]",
        arg_hint: "<filename.json>",
        help_extra_lines: &[],
        usage: "/load [path] — default greatsage-session.json",
        detail: concat!(
            "Usage:\n",
            "  /load\t\t\tLoad from yoyo-session.json\n",
            "  /load <path>\t\tLoad from specified path\n",
            "\n",
            "Restores a previously saved session, replacing the current\n",
            "conversation history.\n",
            "\n",
            "Examples:\n",
            "  /load\n",
            "  /load my-debug-session.json\n",
        ),
    },
    ReplCommand {
        name: "/retry",
        summary: "Re-send the last user input",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/retry — Re-send the last user input",
        detail: concat!(
            "Re-sends the most recent non-slash REPL input to the agent.\n",
            "Prints (retrying last input) before resending.\n",
            "With no prior input, prints (nothing to retry — no previous input).\n",
        ),
    },
    ReplCommand {
        name: "/status",
        summary: "Show session info",
        cli_summary: None,
        short_description: Some("Show session dashboard"),
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/status — Show session info",
        detail: concat!(
            "Displays current session information including: working directory,\n",
            "active model, message count, git branch (if in a repo), and \n",
            "context window usage percentage.\n",
        ),
    },
    ReplCommand {
        name: "/tokens",
        summary: "Show token usage and context window",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/tokens — Show token usage and context window",
        detail: concat!(
            "Displays current token usage (input/output), the model's context\n",
            "window size, remaining-turns estimate, per-category context\n",
            "breakdown, and tool usage summary. Helps you decide when to\n",
            "/compact.\n"
        ),
    },
    ReplCommand {
        name: "/cost",
        summary: "Show estimated session cost",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/cost — Show estimated session cost",
        detail: concat!(
            "Displays the running cost estimate for this session based on\n",
            "input/output tokens and the current model's pricing. Supports\n",
            "cost tracking across multiple providers.\n",
        ),
    },
    ReplCommand {
        name: "/hooks",
        summary: "Show active hooks",
        cli_summary: None,
        short_description: Some("Show active hooks (pre/post tool execution)"),
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/hooks — Show active hooks (pre/post tool execution)",
        detail: concat!(
            "Lists shell hooks from the resolved user config at\n",
            "~/.config/greatsage/config.toml (or $XDG_CONFIG_HOME/greatsage/config.toml).\n",
            "Shows each hook's phase (pre/post), tool pattern, and command.\n",
            "\n",
            "Configuration (config.toml):\n",
            "\n",
            "  # Pre-hook: runs before bash tool calls\n",
            "  hooks.pre.bash = \"echo 'About to run bash'\"\n",
            "\n",
            "  # Post-hook: runs after every tool call (wildcard)\n",
            "  hooks.post.* = \"echo 'Tool finished'\"\n",
            "\n",
            "Pre-hooks that exit non-zero block the tool from executing.\n",
            "Post-hooks always pass through the original tool output.\n",
            "All hooks have a 5-second timeout to prevent hanging.\n",
            "\n",
            "Environment variables available to hooks:\n",
            "  TOOL_NAME   — the tool being executed\n",
            "  TOOL_PARAMS — JSON string of tool parameters\n",
            "  TOOL_OUTPUT — (post-hooks only) tool output, truncated to 1000 chars\n",
        ),
    },
    ReplCommand {
        name: "/history",
        summary: "Show summary of conversation messages",
        cli_summary: Some("Show conversation message summary"),
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "detail",
        help_extra_lines: &[HelpUsageExtraLine {
            label: "/history detail",
            summary: "Per-turn breakdown with tools and token counts",
        }],
        usage: "/history — Show summary of conversation messages",
        detail: concat!(
            "Displays a compact list of all messages in the current\n",
            "conversation: role and a preview of each message.\n",
            "Useful for understanding conversation flow.\n",
            "\n",
            "Subcommands:\n",
            "\n",
            "  /history detail — Per-turn breakdown with tools used and token counts\n",
            "\n",
            "Note: this is conversation history (yoagent messages), not the\n",
            "readline ↑↓ input history stored under ~/.config/greatsage/history.\n",
        ),
    },
    ReplCommand {
        name: "/search",
        summary: "Search conversation history for matching messages",
        cli_summary: Some("Search conversation history"),
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "<query>",
        arg_hint: "<query>",
        help_extra_lines: &[],
        usage: "/search <query> — Search conversation history for matching messages",
        detail: concat!(
            "Usage:\n",
            "  /search <query>\tFind messages containing the query\n",
            "\n",
            "Searches through all conversation messages for matching text\n",
            "(case-insensitive). Shows matching message indices with previews.\n",
            "\n",
            "Examples:\n",
            "  /search error handling\n",
            "  /search TODO\n",
        ),
    },
    ReplCommand {
        name: "/mark",
        summary: "Bookmark current conversation state",
        cli_summary: Some("Bookmark conversation state"),
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "<name>",
        arg_hint: "<name>",
        help_extra_lines: &[],
        usage: "/mark <name> — Bookmark current conversation state",
        detail: concat!(
            "Usage:\n",
            "  /mark <name>\t\tSave an in-memory bookmark at the current point\n",
            "\n",
            "Stores a snapshot of the conversation messages under <name>.\n",
            "Use /jump <name> to restore later. Overwrites an existing name.\n",
            "Bookmarks are process-local and are lost on exit; use /save for durable checkpoints.\n",
        ),
    },
    ReplCommand {
        name: "/jump",
        summary: "Restore conversation to a bookmark (discards messages after it)",
        cli_summary: Some("Restore to a bookmark"),
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "<name>",
        arg_hint: "<name>",
        help_extra_lines: &[],
        usage: "/jump <name> — Restore conversation to a bookmark",
        detail: concat!(
            "Usage:\n",
            "  /jump <name>\t\tRestore the conversation to a saved bookmark\n",
            "\n",
            "Messages added after the bookmark are discarded.\n",
            "Reinstalls the agent (same ECS alignment as /load) and syncs\n",
            "SessionContextStats. Blocked while the session is processing.\n",
        ),
    },
    ReplCommand {
        name: "/marks",
        summary: "List all saved bookmarks",
        cli_summary: Some("List saved bookmarks"),
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/marks — List saved bookmarks",
        detail: concat!(
            "Lists in-memory bookmark names sorted alphabetically.\n",
            "Bookmarks are not persisted across process exit.\n",
        ),
    },
    ReplCommand {
        name: "/export",
        summary: "Export conversation as readable markdown (default: conversation.md)",
        cli_summary: Some("Export conversation as markdown"),
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "[path]",
        arg_hint: "[filename]",
        help_extra_lines: &[],
        usage: "/export [path] — Export conversation as markdown",
        detail: concat!(
            "Usage:\n",
            "  /export\t\tWrite conversation.md in the current directory\n",
            "  /export <path>\tWrite to the specified path\n",
            "\n",
            "Exports the current yoagent conversation as readable markdown.\n",
            "Does nothing when the conversation is empty.\n",
        ),
    },
    ReplCommand {
        name: "/context",
        summary: "Show loaded project context files",
        cli_summary: None,
        short_description: Some("Show project context, system prompt sections, or token budget"),
        category: ReplCommandCategory::Project,
        args: "[system|files]",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/context [system|files] — Show project instruction files and system prompt",
        detail: concat!(
            "Lists project instruction files found in the working directory\n",
            "(e.g. GREATSAGE.md, AGENTS.md) and inspects the assembled system prompt.\n",
            "\n",
            "Subcommands:\n",
            "  /context         List loaded project context files (path + line count)\n",
            "  /context system  Show system prompt sections with token estimates\n",
            "  /context files   List files referenced in this conversation (tool calls)\n",
            "\n",
            "Context window fill and breakdown: use /tokens (not /context tokens).\n",
        ),
    },
    ReplCommand {
        name: "/cd",
        summary: "Change the working directory (~ and relative paths)",
        cli_summary: Some("Change the working directory (~ and relative paths)"),
        short_description: Some("Change the working directory (~ and relative paths supported)"),
        category: ReplCommandCategory::Project,
        args: "<path>",
        arg_hint: "<path>",
        help_extra_lines: &[],
        usage: "/cd <path> — Change the working directory",
        detail: concat!(
            "Usage:\n",
            "  /cd <path>\tChange to the given directory\n",
            "  /cd           Print the current working directory (like pwd)\n",
            "\n",
            "Supports ~ expansion and relative paths. The new directory is\n",
            "used for subsequent commands and tool calls. Project context\n",
            "(YOYO.md, CLAUDE.md, etc.) loaded at startup is NOT reloaded —\n",
            "use /context to review what was loaded.\n",
            "\n",
            "Examples:\n",
            "  /cd ~/projects/myapp\n",
            "  /cd ../sibling\n",
            "  /cd /tmp\n",
        ),
    },
    ReplCommand {
        name: "/init",
        summary: "Generate a GREATSAGE.md project context",
        cli_summary: None,
        short_description: Some("Generate a GREATSAGE.md context file"),
        category: ReplCommandCategory::Project,
        args: "",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/init — Scan project and generate a GREATSAGE.md context file",
        detail: concat!(
            "Analyzes the project structure, detects the tech stack, and\n",
            "creates a GREATSAGE.md file with context information. This file\n",
            "is automatically loaded in future sessions to give the AI\n",
            "project awareness.\n",
            "\n",
            "If GREATSAGE.md already exists, /init refuses to overwrite it.\n",
            "If only YOYO.md or CLAUDE.md is present, /init suggests renaming\n",
            "that file to GREATSAGE.md (yoyo-aligned compat behavior).\n",
            "Use /context to inspect loaded project files.\n",
        ),
    },
    ReplCommand {
        name: "/run",
        summary: "Run a shell command directly (no AI, no tokens)",
        cli_summary: Some("Run a shell command (no AI, no tokens)"),
        short_description: Some("Run shell command (or !<cmd>)"),
        category: ReplCommandCategory::Project,
        args: "<cmd>",
        arg_hint: "<command>",
        help_extra_lines: &[HelpUsageExtraLine {
            label: "!<cmd>",
            summary: "Shortcut for /run",
        }],
        usage: "/run <command> — Run a shell command (no AI, no tokens)",
        detail: concat!(
            "Usage:\n",
            "  /run <command>\tRun via sh -c in the process cwd\n",
            "  !<command>\t\tShortcut for /run (not a slash command)\n",
            "\n",
            "Executes a local shell command without invoking the coding agent\n",
            "or spending model tokens. Prints stdout/stderr and an exit summary.\n",
            "\n",
            "Examples:\n",
            "  /run git status\n",
            "  !ls -la\n",
            "  !git log --oneline -5\n",
        ),
    },
    ReplCommand {
        name: "/model",
        summary: "Switch, list, or inspect models",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Ai,
        args: "<name>",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/model [name] — e.g. /model claude-opus-4-7",
        detail: concat!(
            "Usage:\n",
            "  /model <name>       Switch to the specified model\n",
            "  /model list         Show all available models by provider\n",
            "  /model list <prov>  Show models for a specific provider\n",
            "  /model info [name]  Show details (pricing, context, provider)\n",
            "\n",
            "Changes the active model while preserving the conversation.\n",
            "Tab-completion is available for known model names.\n",
            "\n",
            "Examples:\n",
            "  /model claude-sonnet-4-20250514\n",
            "  /model gpt-4o\n",
            "  /model list\n",
            "  /model list anthropic\n",
            "  /model info gpt-4o",
        ),
    },
    ReplCommand {
        name: "/provider",
        summary: "Switch provider (resets model to provider default)",
        cli_summary: None,
        short_description: Some("Switch or show current provider"),
        category: ReplCommandCategory::Ai,
        args: "<name>",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/provider <name> —  Switch AI provide",
        detail: concat!(
            "Usage:\n",
            "  /provider\t\tShow current provider\n",
            "  /provider list\t\tList providers from the static catalog\n",
            "  /provider <name>\tSwitch to the specified provider\n",
            "\n",
            "Changes the active AI provider and resets the model to that\n",
            "provider's default. Tab-completion is available.\n",
            "\n",
            "Providers: anthropic, openai, google, deepseek, openrouter, local\n",
            "\n",
            "Examples:\n",
            "  /provider\n",
            "  /provider list\n",
            "  /provider openai\n",
            "  /provider google\n",
        ),
    },
    ReplCommand {
        name: "/remember",
        summary: "Save a project-specific memory",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Ai,
        args: "<note>",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/remember <note> — Save a project-specific memory",
        detail: concat!(
            "Usage:\n",
            "  /remember <note>\tSave a memory for this project\n",
            "\n",
            "Saves a note that persists across sessions for the current\n",
            "project directory. Memories are loaded automatically when\n",
            "you start greatsage in the same directory.\n",
            "\n",
            "Examples:\n",
            "  /remember always run migrations before testing\n",
            "  /remember the auth module uses JWT with RS256\n"
        ),
    },
    ReplCommand {
        name: "/memories",
        summary: "List project memories",
        cli_summary: None,
        short_description: Some("List or search project memories"),
        category: ReplCommandCategory::Ai,
        args: "[query]",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/memories [query] — List or search project memories",
        detail: concat!(
            "Usage:\n",
            "  /memories\t\tList all saved memories\n",
            "  /memories <query>\tSearch memories by keyword (case-insensitive)\n",
            "\n",
            "Shows saved memories for the current project directory.\n",
            "Each memory is displayed with its index (for use with /forget),\n",
            "the saved text, and a feed-style timestamp (e.g. 5m ago,\n",
            "Yesterday at 22:00, May 1).\n",
            "\n",
            "Examples:\n",
            "  /memories\n",
            "  /memories docker\n",
            "  /memories sqlx\n"
        ),
    },
    ReplCommand {
        name: "/forget",
        summary: "Remove a project memory by index",
        cli_summary: None,
        short_description: None,
        category: ReplCommandCategory::Ai,
        args: "<n>",
        arg_hint: "",
        help_extra_lines: &[],
        usage: "/forget <n> — Remove a project memory by index",
        detail: concat!(
            "Usage:\n",
            "  /forget <n>\nDelete the memory at the given index\n",
            "\n",
            "Removes a previously saved project memory. Use /memories to\n",
            "see all memories with their indices.\n",
            "\n",
            "Examples:\n",
            "  /forget 0\n",
            "  /forget 3\n"
        ),
    },
];

/// Split [`ReplCommand::detail`] for display.
pub(super) fn detail_output_lines(detail: &str) -> Vec<String> {
    detail.lines().map(str::to_string).collect()
}

fn command_detail_lines(cmd: &ReplCommand) -> Vec<String> {
    detail_output_lines(cmd.detail)
}

fn command_list_label(cmd: &ReplCommand) -> String {
    if cmd.args.is_empty() {
        cmd.name.to_string()
    } else {
        format!("{} {}", cmd.name, cmd.args)
    }
}

fn primary_summary(cmd: &ReplCommand, audience: HelpListAudience) -> &'static str {
    match audience {
        HelpListAudience::Repl => cmd.summary,
        HelpListAudience::Cli => cmd.cli_summary.unwrap_or(cmd.summary),
    }
}

fn format_help_list_line(label: &str, summary: &str, label_width: usize) -> String {
    format!("  {label:<label_width$}  {summary}")
}

fn command_list_label_width() -> usize {
    let mut max_w = 0usize;
    for cmd in KNOWN_COMMANDS {
        max_w = max_w.max(command_list_label(cmd).chars().count());
        for extra in cmd.help_extra_lines {
            max_w = max_w.max(extra.label.chars().count());
        }
    }
    max_w
}

/// Keep fenced CLI help under common 80-col terminals.
///
/// termimad pads every code-fence line to the block's widest line; if that
/// width exceeds the terminal, each line wraps and the pad looks like a blank.
const CLI_HELP_CODE_WIDTH: usize = 78;

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn split_at_chars(s: &str, n: usize) -> (&str, &str) {
    match s.char_indices().nth(n) {
        Some((i, _)) => (&s[..i], &s[i..]),
        None => (s, ""),
    }
}

/// Summary column for `format_help_list_line`: `"  " + label_width + "  "`.
///
/// Must not scan the line for `"  "` — labels like `/search <query>` contain
/// spaces, and label padding is also spaces; a naive find mis-aligns wraps.
fn summary_column_indent(label_width: usize) -> usize {
    2 + label_width + 2
}

/// Wrap a single help list line so its display width is ≤ `max_width` chars.
/// Continuation lines indent to the summary column (`label_width`).
fn wrap_cli_help_line(line: &str, max_width: usize, label_width: usize) -> Vec<String> {
    if line.is_empty() || char_len(line) <= max_width {
        return vec![line.to_string()];
    }

    // Category headers (`── Session ──`) have no summary column; indent modestly.
    let indent = if line.trim_start().starts_with('─') {
        2usize
    } else {
        summary_column_indent(label_width).min(max_width.saturating_sub(8))
    };
    let indent_s = " ".repeat(indent);
    let mut out = Vec::new();
    let mut rest = line;
    let mut first = true;

    while !rest.is_empty() {
        let budget = if first {
            max_width
        } else {
            max_width.saturating_sub(indent)
        };
        if char_len(rest) <= budget {
            if first {
                out.push(rest.to_string());
            } else {
                out.push(format!("{indent_s}{rest}"));
            }
            break;
        }

        let prefix = split_at_chars(rest, budget).0;
        // Prefer breaking on a space (not mid-word) when possible.
        let break_at = prefix
            .char_indices()
            .rev()
            .find(|&(_, c)| c == ' ')
            .map(|(i, _)| i)
            .filter(|&i| i > 0)
            .unwrap_or_else(|| prefix.len());

        let (chunk, rem) = if break_at < rest.len() {
            let (a, b) = rest.split_at(break_at);
            (a.trim_end(), b.trim_start())
        } else {
            let (a, b) = split_at_chars(rest, budget);
            (a, b)
        };

        if first {
            out.push(chunk.to_string());
            first = false;
        } else {
            out.push(format!("{indent_s}{chunk}"));
        }
        rest = rem;
        if rest.is_empty() {
            break;
        }
    }
    out
}

fn wrap_cli_help_body(body: &str, max_width: usize, label_width: usize) -> String {
    body.lines()
        .flat_map(|line| wrap_cli_help_line(line, max_width, label_width))
        .collect::<Vec<_>>()
        .join("\n")
}

fn command_list_entries_for_category(
    category: ReplCommandCategory,
    label_width: usize,
    audience: HelpListAudience,
) -> Vec<String> {
    let mut entries = Vec::new();
    for cmd in KNOWN_COMMANDS.iter().filter(|cmd| cmd.category == category) {
        entries.push(format_help_list_line(
            &command_list_label(cmd),
            primary_summary(cmd, audience),
            label_width,
        ));
        for extra in cmd.help_extra_lines {
            entries.push(format_help_list_line(
                extra.label,
                extra.summary,
                label_width,
            ));
        }
    }
    entries
}

fn repl_command_lines_grouped(audience: HelpListAudience) -> String {
    let label_width = command_list_label_width();
    ReplCommandCategory::ALL
        .iter()
        .filter_map(|category| {
            let entries = command_list_entries_for_category(*category, label_width, audience);
            if entries.is_empty() {
                return None;
            }
            Some(format!(
                "  {}\n{}",
                category.section_header(),
                entries.join("\n")
            ))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn normalize_command_name(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

#[allow(dead_code)]
pub fn command_usage(name: &str) -> Option<&'static str> {
    let normalized = normalize_command_name(name);
    KNOWN_COMMANDS
        .iter()
        .find(|cmd| cmd.name == normalized)
        .map(|cmd| cmd.usage)
}

fn inline_hint_description(cmd: &ReplCommand) -> &'static str {
    cmd.short_description.unwrap_or(cmd.summary)
}

/// Short description for inline ghost hints (yoyo `command_short_description`).
///
/// Returns [`ReplCommand::short_description`] when set; otherwise [`ReplCommand::summary`].
pub fn command_short_description(cmd_name: &str) -> Option<&'static str> {
    let normalized = normalize_command_name(cmd_name);
    KNOWN_COMMANDS
        .iter()
        .find(|cmd| cmd.name == normalized)
        .map(inline_hint_description)
}

/// Argument hint for inline ghost text after `cmd ` (name with or without `/`).
/// Prefer explicit [`ReplCommand::arg_hint`] so list labels can stay bare
/// (`args: ""`) while still showing a ghost (e.g. `/history ` → `detail`).
pub(super) fn command_arg_hint(cmd_name: &str) -> Option<&'static str> {
    let normalized = normalize_command_name(cmd_name);
    let cmd = KNOWN_COMMANDS.iter().find(|c| c.name == normalized)?;
    // yoyo omits ghost hints for optional-flag commands (e.g. bare `/compact `).
    if normalized == "/compact" {
        return None;
    }
    if !cmd.arg_hint.is_empty() {
        return Some(cmd.arg_hint);
    }
    if cmd.args.is_empty() {
        None
    } else {
        Some(cmd.args)
    }
}

#[allow(dead_code)]
pub fn command_detail(name: &str) -> Option<&'static str> {
    let normalized = normalize_command_name(name);
    KNOWN_COMMANDS
        .iter()
        .find(|cmd| cmd.name == normalized)
        .map(|cmd| cmd.detail)
}

#[allow(dead_code)]
pub(super) fn format_help_detail(name: &str) -> String {
    let normalized = normalize_command_name(name);
    let Some(cmd) = KNOWN_COMMANDS.iter().find(|c| c.name == normalized) else {
        return format!("Unknown command: {name}\nTry /help for a list of available commands.");
    };
    let detail = command_detail_lines(cmd);
    if detail.is_empty() {
        cmd.usage.to_string()
    } else {
        format!("{}\n\n{}", cmd.usage, detail.join("\n"))
    }
}

pub(super) fn format_help_detail_parts(name: &str) -> (String, Vec<String>) {
    let normalized = normalize_command_name(name);
    let Some(cmd) = KNOWN_COMMANDS.iter().find(|c| c.name == normalized) else {
        return (
            format!("Unknown command: {name}\nTry /help for a list of available commands."),
            Vec::new(),
        );
    };
    (cmd.usage.to_string(), command_detail_lines(cmd))
}

pub(super) fn push_usage_and_body_parts(command: &str) -> (String, Vec<String>) {
    let normalized = normalize_command_name(command);
    let Some(cmd) = KNOWN_COMMANDS.iter().find(|c| c.name == normalized) else {
        return (String::new(), Vec::new());
    };
    (cmd.usage.to_string(), command_detail_lines(cmd))
}

pub(super) fn push_usage_and_body(lines: &mut Vec<String>, command: &str) {
    let (usage, detail) = push_usage_and_body_parts(command);
    if !usage.is_empty() {
        () = lines.push(usage);
    }
    lines.extend(detail);
}

pub(super) fn help_text() -> String {
    // Same wrap as CLI: long summaries break mid-line on 80-col terminals and
    // look misaligned; continuation lines indent to the summary column.
    let label_width = command_list_label_width();
    let body = wrap_cli_help_body(
        &repl_command_lines_grouped(HelpListAudience::Repl),
        CLI_HELP_CODE_WIDTH,
        label_width,
    );
    format!("REPL commands (try /help <command> for details):\n{body}")
}

pub(super) fn help_text_lines() -> Vec<String> {
    help_text().lines().map(str::to_string).collect()
}

pub(crate) fn cli_repl_commands_section() -> String {
    // Use a ``` fence so termimad applies code_block colors (the grey panel).
    // Pre-wrap long lines to CLI_HELP_CODE_WIDTH so CodeBlock::justify pads to a
    // width that fits common 80-col terminals — otherwise every line wraps and
    // the pad looks like a blank row after each command.
    // Continuation indent uses the same summary column as format_help_list_line
    // (`2 + label_width + 2`), not a scan for `"  "` (labels contain spaces).
    let label_width = command_list_label_width();
    let body = wrap_cli_help_body(
        &repl_command_lines_grouped(HelpListAudience::Cli),
        CLI_HELP_CODE_WIDTH,
        label_width,
    );
    format!("\n**Commands (in REPL):**\n\n```\n{body}\n```\n")
}

/// Status line, usage, detail body, and pointer to full help.
#[allow(dead_code)]
pub fn format_command_context(command: &str, status: &str) -> Vec<String> {
    let mut lines = vec![status.to_string()];
    () = push_usage_and_body(&mut lines, command);
    () = lines.push(format!("Try /help {command} for details."));
    lines
}

#[allow(dead_code)]
pub fn usage_only(command: &str) -> Vec<String> {
    let mut lines = Vec::new();
    () = push_usage_and_body(&mut lines, command);
    () = lines.push(format!("Try /help {command} for details."));
    lines
}
