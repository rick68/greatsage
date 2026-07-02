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

#[derive(Debug, Clone, Copy)]
pub struct ReplCommand {
    pub name: &'static str,
    pub summary: &'static str,
    /// Inline ghost hint override; [`None`] uses [`Self::summary`].
    pub short_description: Option<&'static str>,
    pub category: ReplCommandCategory,
    /// Argument synopsis in `/help` list (`[opt]`, `<required>`, or empty).
    pub args: &'static str,
    /// Inline ghost hint after `cmd `; uses [`Self::args`] when empty.
    pub arg_hint: &'static str,
    pub usage: &'static str,
    /// Explanatory body for `/help <cmd>` and error hints; does not repeat [`Self::usage`].
    /// Prefer [`concat!`] for multi-line text so Rust source indent does not leak into output.
    pub detail: &'static str,
}

pub const KNOWN_COMMANDS: &[ReplCommand] = &[
    ReplCommand {
        name: "/help",
        summary: "Show this help",
        short_description: Some("Show help for commands"),
        category: ReplCommandCategory::Session,
        args: "[command]",
        arg_hint: "",
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
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        usage: "/quit  (alias: /exit)",
        detail: QUIT_EXIT_DETAIL,
    },
    ReplCommand {
        name: "/exit",
        summary: "Exit greatsage (alias for /quit)",
        short_description: Some("Exit greatsage"),
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        usage: "/exit  (alias: /quit)",
        detail: QUIT_EXIT_DETAIL,
    },
    ReplCommand {
        name: "/clear",
        summary: "Clear conversation history",
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
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
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
        usage: "/clear! — Force-clear conversation history",
        detail: concat!(
            "Same as /clear but skips the confirmation prompt.\n",
            "Always clears immediately regardless of message count.\n",
        ),
    },
    ReplCommand {
        name: "/compact",
        summary: "Compact conversation to save context",
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "[N|all|--preview]",
        arg_hint: "",
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
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "[path]",
        arg_hint: "<filename.json>",
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
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "[path]",
        arg_hint: "<filename.json>",
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
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
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
        short_description: Some("Show session dashboard"),
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
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
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
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
        short_description: None,
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
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
        short_description: Some("Show active hooks (pre/post tool execution)"),
        category: ReplCommandCategory::Session,
        args: "",
        arg_hint: "",
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
        name: "/context",
        summary: "Show loaded project context files",
        short_description: Some("Show project context, system prompt sections, or token budget"),
        category: ReplCommandCategory::Project,
        args: "[system|files]",
        arg_hint: "",
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
        name: "/init",
        summary: "Generate a GREATSAGE.md project context",
        short_description: Some("Generate a GREATSAGE.md context file"),
        category: ReplCommandCategory::Project,
        args: "",
        arg_hint: "",
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
        name: "/model",
        summary: "Switch, list, or inspect models",
        short_description: None,
        category: ReplCommandCategory::Ai,
        args: "<name>",
        arg_hint: "",
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
        short_description: Some("Switch or show current provider"),
        category: ReplCommandCategory::Ai,
        args: "<name>",
        arg_hint: "",
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
        short_description: None,
        category: ReplCommandCategory::Ai,
        args: "<note>",
        arg_hint: "",
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
        short_description: Some("List or search project memories"),
        category: ReplCommandCategory::Ai,
        args: "[query]",
        arg_hint: "",
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
        short_description: None,
        category: ReplCommandCategory::Ai,
        args: "<n>",
        arg_hint: "",
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

fn command_list_label_width() -> usize {
    KNOWN_COMMANDS
        .iter()
        .map(|cmd| command_list_label(cmd).chars().count())
        .max()
        .unwrap_or(0)
}

fn command_list_entry(cmd: &ReplCommand, label_width: usize) -> String {
    let label = command_list_label(cmd);
    format!("  {label:<label_width$}  {}", cmd.summary)
}

fn repl_command_lines_grouped() -> String {
    let label_width = command_list_label_width();
    ReplCommandCategory::ALL
        .iter()
        .filter_map(|category| {
            let entries: Vec<_> = KNOWN_COMMANDS
                .iter()
                .filter(|cmd| cmd.category == *category)
                .map(|cmd| command_list_entry(cmd, label_width))
                .collect();
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
pub(super) fn command_arg_hint(cmd_name: &str) -> Option<&'static str> {
    let normalized = normalize_command_name(cmd_name);
    let cmd = KNOWN_COMMANDS.iter().find(|c| c.name == normalized)?;
    if cmd.args.is_empty() {
        return None;
    }
    // yoyo omits ghost hints for optional-flag commands (e.g. bare `/compact `).
    if normalized == "/compact" {
        return None;
    }
    if cmd.arg_hint.is_empty() {
        Some(cmd.args)
    } else {
        Some(cmd.arg_hint)
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
    format!(
        "REPL commands (try /help <command> for details):\n{}",
        repl_command_lines_grouped()
    )
}

pub(super) fn help_text_lines() -> Vec<String> {
    help_text().lines().map(str::to_string).collect()
}

pub(crate) fn cli_repl_commands_section() -> String {
    // Fenced block preserves category/command indentation through clap_help markdown.
    format!(
        "\n**Commands (in REPL):**\n\n```\n{}\n```\n",
        repl_command_lines_grouped()
    )
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
