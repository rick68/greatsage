//! REPL/agent startup hint lines (banner, config, context, model, …).

use {
    crate::{config_paths::config_hint_line, project_context::project_context_hint_lines},
    colored::Colorize,
    std::path::Path,
};

pub struct StartupHintInput<'a> {
    pub bare: bool,
    pub no_hints: bool,
    pub print_system_prompt: bool,
    pub cwd: &'a Path,
    pub config_path: &'a Path,
    pub model: &'a str,
    pub skills_len: usize,
    pub mcp_len: usize,
    pub needs_setup: bool,
}

pub enum StartupHintPart {
    Banner(String),
    Dimmed(String),
}

pub fn startup_hints_enabled(input: &StartupHintInput<'_>) -> bool {
    !input.no_hints && !input.print_system_prompt
}

fn banner() -> String {
    format!(
        "\n{} {}\n",
        <&str as Colorize>::bold("greatsage").cyan(),
        "— a coding agent growing up in public".dimmed()
    )
}

/// Dimmed startup lines (each includes trailing `\n`). Excludes the styled banner.
pub fn startup_hint_dimmed_lines(input: &StartupHintInput<'_>) -> Vec<String> {
    if !startup_hints_enabled(input) {
        return vec![];
    }

    let mut lines = vec![format!(
        "{}\n",
        config_hint_line(input.config_path, input.cwd)
    )];

    if !input.bare {
        for line in project_context_hint_lines(input.cwd) {
            lines.push(format!("{line}\n"));
        }
    }

    () = lines.push(format!("  model: {}\n", input.model));

    if !input.bare {
        if input.skills_len > 0 {
            () = lines.push(format!("  skills: {} loaded\n", input.skills_len));
        }
        if input.mcp_len > 0 {
            () = lines.push(format!("  mcp: {} server(s) connected\n", input.mcp_len));
        }
    }

    () = lines.push(format!("  cwd: {}\n", input.cwd.display()));

    if input.needs_setup {
        () = lines.push("  hint: no API key configured — run `greatsage setup`\n".to_owned());
    }

    lines
}

pub fn startup_hint_parts(input: &StartupHintInput<'_>) -> Vec<StartupHintPart> {
    if !startup_hints_enabled(input) {
        return vec![];
    }

    let mut parts = vec![StartupHintPart::Banner(banner())];
    for line in startup_hint_dimmed_lines(input) {
        () = parts.push(StartupHintPart::Dimmed(line));
    }
    parts
}
