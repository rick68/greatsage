//! Project instruction files from cwd appended to the agent system prompt.

use {
    crate::project_memory::{format_memories_for_prompt, load_memories_from, memory_file_path},
    std::{fs, path::Path},
};

pub const PROJECT_CONTEXT_FILES: &[&str] = &[
    "GREATSAGE.md",
    ".greatsage/instructions.md",
    "AGENTS.md",
    "CLAUDE.md",
    "YOYO.md",
    ".cursorrules",
    ".github/copilot-instructions.md",
];

fn read_nonempty_file(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub fn list_project_context_files(cwd: &Path) -> Vec<(String, usize)> {
    PROJECT_CONTEXT_FILES
        .iter()
        .filter_map(|rel| {
            let path = cwd.join(rel);
            let content = read_nonempty_file(&path)?;
            let line_count = content.lines().count();
            Some(((*rel).to_owned(), line_count))
        })
        .collect()
}

pub fn loaded_project_context_paths(cwd: &Path) -> Vec<&'static str> {
    PROJECT_CONTEXT_FILES
        .iter()
        .copied()
        .filter(|rel| read_nonempty_file(&cwd.join(rel)).is_some())
        .collect()
}

pub fn load_project_context(cwd: &Path) -> Option<String> {
    let mut parts = Vec::new();
    let mut first = true;

    for rel in PROJECT_CONTEXT_FILES {
        if let Some(content) = read_nonempty_file(&cwd.join(rel)) {
            if first {
                () = parts.push(content);
                first = false;
            } else {
                () = parts.push(format!("--- From {rel} ---\n{content}"));
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

pub fn assemble_system_prompt(base: &str, cwd: &Path) -> (String, Vec<&'static str>) {
    let loaded_paths = loaded_project_context_paths(cwd);
    let base_trimmed = base.trim();
    let project = load_project_context(cwd);

    let mut full = match (base_trimmed.is_empty(), project) {
        (true, Some(project)) => project,
        (false, Some(project)) => format!("{base_trimmed}\n\n{project}"),
        (false, None) => base_trimmed.to_owned(),
        (true, None) => String::new(),
    };

    let memory = load_memories_from(&memory_file_path(cwd));
    if let Some(memories_section) = format_memories_for_prompt(&memory) {
        full = if full.is_empty() {
            memories_section
        } else {
            format!("{full}\n\n{memories_section}")
        };
    }

    (full, loaded_paths)
}

pub fn project_context_hint_lines(cwd: &Path) -> Vec<String> {
    loaded_project_context_paths(cwd)
        .into_iter()
        .map(|path| format!("  context: {path}"))
        .collect()
}
