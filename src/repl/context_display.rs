//! Pure formatters for `/context` output (testable without REPL dispatch).

use {
    crate::project_context::{list_project_context_files, load_project_context},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::Path,
    },
    yoagent::types::{AgentMessage, Content, Message},
};

const FROM_PREFIX: &str = "--- From ";
const FROM_SUFFIX: &str = " ---";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSection {
    pub name: String,
    pub header_level: usize,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FileAction {
    Read,
    Edited,
    Written,
    Listed,
    Searched,
}

impl FileAction {
    fn label(self) -> &'static str {
        match self {
            Self::Read => "Read",
            Self::Edited => "Edited",
            Self::Written => "Written",
            Self::Listed => "Listed",
            Self::Searched => "Searched",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Read => "📖",
            Self::Edited => "✏️ ",
            Self::Written => "📝",
            Self::Listed => "📂",
            Self::Searched => "🔍",
        }
    }
}

fn parse_from_separator_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with(FROM_PREFIX) || !trimmed.ends_with(FROM_SUFFIX) {
        return None;
    }
    let inner = trimmed
        .strip_prefix(FROM_PREFIX)?
        .strip_suffix(FROM_SUFFIX)?
        .trim();
    if inner.is_empty() {
        None
    } else {
        Some(String::from(inner))
    }
}

fn parse_markdown_sections_text(text: &str) -> Vec<PromptSection> {
    let mut sections = Vec::new();
    let mut current_name = String::from("(preamble)");
    let mut current_level = 0usize;
    let mut current_lines: Vec<String> = Vec::new();

    let flush = |name: &mut String,
                 level: &mut usize,
                 lines: &mut Vec<String>,
                 sections: &mut Vec<PromptSection>| {
        if !lines.is_empty() || *name != "(preamble)" {
            sections.push(PromptSection {
                name: name.clone(),
                header_level: *level,
                lines: lines.clone(),
            });
        }
        () = lines.clear();
    };

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# ") {
            flush(
                &mut current_name,
                &mut current_level,
                &mut current_lines,
                &mut sections,
            );
            current_name = String::from(rest.trim());
            current_level = 1;
        } else if let Some(rest) = line.strip_prefix("## ") {
            flush(
                &mut current_name,
                &mut current_level,
                &mut current_lines,
                &mut sections,
            );
            current_name = String::from(rest.trim());
            current_level = 2;
        } else {
            () = current_lines.push(String::from(line));
        }
    }
    flush(
        &mut current_name,
        &mut current_level,
        &mut current_lines,
        &mut sections,
    );
    sections
}

pub fn parse_prompt_sections(prompt: &str) -> Vec<PromptSection> {
    if prompt.trim().is_empty() {
        return Vec::new();
    }

    let mut sections = Vec::new();
    let mut pending_name: Option<String> = None;
    let mut lines: Vec<String> = Vec::new();

    let flush_chunk =
        |lines: &mut Vec<String>, name: Option<String>, sections: &mut Vec<PromptSection>| {
            if lines.is_empty() && name.is_none() {
                return;
            }
            let text = lines.join("\n");
            match name {
                Some(section_name) => sections.push(PromptSection {
                    name: section_name,
                    header_level: 0,
                    lines: text.lines().map(str::to_string).collect(),
                }),
                None => sections.extend(parse_markdown_sections_text(&text)),
            }
            () = lines.clear();
        };

    for line in prompt.lines() {
        if let Some(path) = parse_from_separator_line(line) {
            flush_chunk(&mut lines, pending_name.take(), &mut sections);
            pending_name = Some(path);
        } else {
            () = lines.push(String::from(line));
        }
    }
    () = flush_chunk(&mut lines, pending_name.take(), &mut sections);
    sections
}

fn extract_context_files(messages: &[AgentMessage]) -> BTreeMap<FileAction, BTreeSet<String>> {
    let mut result: BTreeMap<FileAction, BTreeSet<String>> = BTreeMap::new();

    for msg in messages {
        let content = match msg {
            AgentMessage::Llm(Message::Assistant { content, .. }) => content,
            _ => continue,
        };
        for block in content {
            let Content::ToolCall {
                name, arguments, ..
            } = block
            else {
                continue;
            };
            let action = match name.as_str() {
                "read_file" => FileAction::Read,
                "edit_file" => FileAction::Edited,
                "write_file" => FileAction::Written,
                "list_files" => FileAction::Listed,
                "search" => FileAction::Searched,
                _ => continue,
            };
            let Some(path) = arguments.get("path").and_then(|v| v.as_str()) else {
                continue;
            };
            if path.is_empty() {
                continue;
            }
            result.entry(action).or_default().insert(String::from(path));
        }
    }

    result
}

fn truncate_preview(line: &str, max_chars: usize) -> String {
    let trimmed = line.trim();
    if trimmed.chars().count() <= max_chars {
        String::from(trimmed)
    } else {
        let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
        () = out.push('…');
        out
    }
}

fn pluralize_lines(count: usize) -> &'static str {
    if count == 1 { "line" } else { "lines" }
}

pub fn context_list_lines(cwd: &Path, system_prompt: &str, bare: bool) -> Vec<String> {
    if bare {
        return vec![
            String::from("Project context disabled (--bare)."),
            String::from("Project instruction files and memories are not loaded into the agent."),
        ];
    }

    let files = list_project_context_files(cwd);
    if files.is_empty() {
        return vec![
            String::from("No project context files found."),
            String::from("Create GREATSAGE.md to give greatsage project context."),
            String::from(
                "Also supports: .greatsage/instructions.md, AGENTS.md, CLAUDE.md, YOYO.md, .cursorrules, .github/copilot-instructions.md",
            ),
        ];
    }

    let mut lines = vec![String::from("Project context files:")];
    for (name, line_count) in &files {
        () = lines.push(format!(
            "{name} ({line_count} {})",
            pluralize_lines(*line_count)
        ));
    }
    if let Some(hint) = reinstall_hint(system_prompt, cwd) {
        () = lines.push(hint);
    }
    lines
}

fn reinstall_hint(system_prompt: &str, cwd: &Path) -> Option<String> {
    let files = list_project_context_files(cwd);
    if files.is_empty() {
        return None;
    }

    let mut stale = false;
    for (path, _) in files.iter().skip(1) {
        if !system_prompt.contains(&format!("--- From {path} ---")) {
            stale = true;
            break;
        }
    }
    if !stale && files.len() == 1 {
        if let Some(block) = load_project_context(cwd) {
            if !system_prompt.contains(block.trim()) {
                stale = true;
            }
        }
    }

    if stale {
        Some(String::from(
            "hint: disk has project files not in the installed prompt — reinstall agent to pick up changes",
        ))
    } else {
        None
    }
}

pub fn context_system_lines(prompt: &str, estimate_tokens: impl Fn(&str) -> usize) -> Vec<String> {
    if prompt.trim().is_empty() {
        return vec![String::from("System prompt is empty.")];
    }

    let sections = parse_prompt_sections(prompt);
    if sections.is_empty() {
        return vec![String::from("System prompt is empty.")];
    }

    let total_lines: usize = sections.iter().map(|s| s.lines.len() + 1).sum();
    let total_tokens = estimate_tokens(prompt);

    let mut lines = vec![String::from("System prompt sections:"), String::new()];
    for section in &sections {
        let section_text = section.lines.join("\n");
        let tokens = estimate_tokens(&format!("{}\n{section_text}", section.name));
        let line_count = section.lines.len();
        let prefix = if section.header_level <= 1 { "#" } else { "##" };
        () = lines.push(format!(
            "{prefix} {}  ({line_count} {}, ~{tokens} tokens)",
            section.name,
            pluralize_lines(line_count),
        ));

        let preview_lines: Vec<&String> = section
            .lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .take(3)
            .collect();
        for line in preview_lines {
            () = lines.push(truncate_preview(line, 80));
        }
        if section
            .lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .count()
            > 3
        {
            () = lines.push(String::from("..."));
        }
        () = lines.push(String::new());
    }
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    () = lines.push(format!(
        "Total: {total_lines} lines, ~{total_tokens} tokens (estimated)"
    ));
    lines
}

pub fn context_files_lines(messages: &[AgentMessage]) -> Vec<String> {
    let files = extract_context_files(messages);
    if files.is_empty() {
        return vec![String::from("(no files referenced yet)")];
    }

    let mut lines = vec![String::from("Files in this conversation:"), String::new()];
    for (action, paths) in &files {
        let joined = paths
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        () = lines.push(format!(
            "{} {:<9} {joined}",
            action.icon(),
            format!("{}:", action.label()),
        ));
    }
    lines
}
