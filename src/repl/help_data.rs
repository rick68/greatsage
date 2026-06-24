#[derive(Debug, Clone, Copy)]
pub struct ReplCommand {
    pub name: &'static str,
    pub summary: &'static str,
    pub detail: &'static str,
}

pub const KNOWN_COMMANDS: &[ReplCommand] = &[
    ReplCommand {
        name: "/help",
        summary: "List commands or show detail for one command",
        detail: "Usage: /help [command]\n\n\
                 Without arguments, lists every REPL slash command with a short summary.\n\
                 With a command name (e.g. /help /model), prints detailed help for that command.",
    },
    ReplCommand {
        name: "/quit",
        summary: "Exit the agent",
        detail: "Exits the REPL and shuts down greatsage. Alias: /exit.",
    },
    ReplCommand {
        name: "/exit",
        summary: "Exit the agent (alias for /quit)",
        detail: "Exits the REPL and shuts down greatsage. Same as /quit.",
    },
    ReplCommand {
        name: "/clear",
        summary: "Clear conversation history",
        detail: "Reinstalls the coding agent with the current model and provider, \
                 resetting the in-memory conversation.",
    },
    ReplCommand {
        name: "/model",
        summary: "Switch model mid-session",
        detail: "Usage: /model <name>\n\n\
                 Switches to the named model for subsequent requests without clearing \
                 conversation history. Example: /model claude-opus-4-7",
    },
];

pub fn repl_command_lines() -> String {
    KNOWN_COMMANDS
        .iter()
        .map(|cmd| format!("  {:<16} {}", cmd.name, cmd.summary))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn help_text() -> String {
    repl_command_lines()
}

pub fn cli_repl_commands_section() -> String {
    format!(
        "\n**Commands (in REPL):**\n{}\n",
        repl_command_lines()
    )
}

pub fn normalize_command_name(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

pub fn command_detail(name: &str) -> Option<&'static str> {
    let normalized = normalize_command_name(name);
    KNOWN_COMMANDS
        .iter()
        .find(|cmd| cmd.name == normalized)
        .map(|cmd| cmd.detail)
}

pub fn format_help_detail(name: &str) -> String {
    if let Some(detail) = command_detail(name) {
        return detail.to_string();
    }
    format!(
        "Unknown command: {name}\nTry /help for a list of available commands."
    )
}