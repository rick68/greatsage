//! Tab-completion candidates, inline ghost hints, and token editing helpers.
//!
//! File entry last; each function directly above its callers. Local callees sit
//! immediately above their caller in source appearance order; reuse earlier defs.

use {
    super::{
        dispatch::command_name_and_args,
        help_data::{KNOWN_COMMANDS, command_arg_hint as help_arg_hint},
    },
    crate::{agents::AgentConfig, providers::PROVIDER_SPECS},
    std::{
        fs, iter,
        path::{MAIN_SEPARATOR, Path},
    },
    unicode_width::UnicodeWidthChar,
};

pub(super) fn str_display_width(s: &str) -> usize {
    s.chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum()
}

fn path_enters_subdirectory(partial: &str) -> bool {
    partial.ends_with('/') || partial.ends_with(MAIN_SEPARATOR)
}

fn path_like_partial(partial: &str) -> bool {
    partial.contains('/') || partial.starts_with('.')
}

fn session_path_dotfile_browse(partial: &str) -> bool {
    partial == "." || partial.ends_with("/.")
}

fn path_filename_component(partial: &str) -> String {
    if partial.ends_with('/') || partial.ends_with(MAIN_SEPARATOR) {
        return String::new();
    }
    Path::new(partial)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Character index bounds `[start, end)` of the token being completed at `cursor` (in chars).
pub(super) fn token_bounds(line: &str, cursor: usize) -> (usize, usize) {
    let cursor = cursor.min(line.chars().count());
    let end_byte = line
        .char_indices()
        .nth(cursor)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    let head = &line[..end_byte];
    let start_byte = head.rfind(' ').map(|i| i + 1).unwrap_or(0);
    let start_char = line[..start_byte].chars().count();
    (start_char, cursor)
}

pub(super) fn token_at_bounds(line: &str, start_char: usize, end_char: usize) -> String {
    line.chars()
        .skip(start_char)
        .take(end_char.saturating_sub(start_char))
        .collect()
}

pub fn apply_replacement(
    line: &str,
    start_char: usize,
    end_char: usize,
    replacement: &str,
) -> String {
    let start_byte = line
        .char_indices()
        .nth(start_char)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    let end_byte = line
        .char_indices()
        .nth(end_char)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    let mut out = String::with_capacity(line.len() + replacement.len());
    () = out.push_str(&line[..start_byte]);
    () = out.push_str(replacement);
    () = out.push_str(&line[end_byte..]);
    out
}

pub fn common_prefix(candidates: &[String]) -> String {
    if candidates.is_empty() {
        return String::new();
    }
    if candidates.len() == 1 {
        return candidates[0].clone();
    }
    let mut prefix = candidates[0].clone();
    for cand in &candidates[1..] {
        while !cand.starts_with(&prefix) {
            prefix.pop();
            if prefix.is_empty() {
                return String::new();
            }
        }
    }
    prefix
}

fn complete_commands(prefix: &str) -> Vec<String> {
    let mut out: Vec<String> = KNOWN_COMMANDS
        .iter()
        .filter(|cmd| cmd.name.starts_with(prefix))
        .map(|cmd| cmd.name.to_string())
        .collect();
    () = out.sort();
    () = out.dedup();
    out
}

fn complete_provider_names(prefix: &str) -> Vec<String> {
    let lower = prefix.to_lowercase();
    let mut out: Vec<String> = PROVIDER_SPECS
        .iter()
        .map(|spec| spec.provider.to_string())
        .filter(|name| name.starts_with(&lower))
        .collect();
    () = out.sort();
    () = out.dedup();
    out
}

fn complete_model_names(prefix: &str, agent_config: &AgentConfig) -> Vec<String> {
    let models = agent_config.provider.known_models();
    if models.is_empty() {
        return Vec::new();
    }
    let lower = prefix.to_lowercase();
    let mut out: Vec<String> = models
        .iter()
        .copied()
        .filter(|m| m.to_lowercase().starts_with(&lower))
        .map(str::to_string)
        .collect();
    if out.is_empty() && prefix.is_empty() {
        out = models.iter().copied().map(str::to_string).collect();
    }
    () = out.sort();
    () = out.dedup();
    out
}

fn complete_all_model_names(prefix: &str) -> Vec<String> {
    let lower = prefix.to_lowercase();
    let mut out: Vec<String> = PROVIDER_SPECS
        .iter()
        .flat_map(|spec| spec.known_models.iter().copied())
        .filter(|m| m.to_lowercase().starts_with(&lower))
        .map(str::to_string)
        .collect();
    () = out.sort();
    () = out.dedup();
    out
}

fn complete_model_subcommands(prefix: &str) -> Vec<String> {
    let lower = prefix.to_lowercase();
    ["info", "list"]
        .iter()
        .filter(|sub| sub.starts_with(&lower))
        .map(|sub| (*sub).to_string())
        .collect()
}

fn complete_model_line_args(line: &str, prefix: &str, agent_config: &AgentConfig) -> Vec<String> {
    let (_, args) = command_name_and_args(line);
    let sub = args.trim();
    if sub == "list" || sub.starts_with("list ") {
        return complete_provider_names(prefix);
    }
    if sub == "info" || sub.starts_with("info ") {
        return complete_all_model_names(prefix);
    }
    let mut out = complete_model_subcommands(prefix);
    out.extend(complete_model_names(prefix, agent_config));
    () = out.sort();
    () = out.dedup();
    out
}

pub fn token_prefix(line: &str, cursor: usize) -> &str {
    let (start, end) = token_bounds(line, cursor);
    let start_byte = line
        .char_indices()
        .nth(start)
        .map(|(i, _)| i)
        .unwrap_or_else(|| line.len());
    let end_byte = line
        .char_indices()
        .nth(end)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    &line[start_byte..end_byte]
}

fn ghost_suffix(prefix: &str, candidates: &[String]) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        let cand = &candidates[0];
        if cand.starts_with(prefix) && cand != prefix {
            return Some(cand[prefix.len()..].to_string());
        }
        return None;
    }
    let shared = common_prefix(candidates);
    if shared.len() > prefix.len() {
        return Some(shared[prefix.len()..].to_string());
    }
    None
}

/// Lay out tab-completion candidates in readline-style columns (top-to-bottom, then across).
pub fn format_candidate_columns(candidates: &[String], term_width: usize) -> Vec<String> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let term_width = term_width.max(20);
    let max_width = candidates
        .iter()
        .map(|c| str_display_width(c))
        .max()
        .unwrap_or(0);
    let col_width = max_width + 1;
    let ncol = (term_width / col_width).max(1);
    let nrow = candidates.len().div_ceil(ncol);

    let mut lines = Vec::with_capacity(nrow);
    for row in 0..nrow {
        let mut line = String::new();
        for col in 0..ncol {
            let idx = col * nrow + row;
            if idx >= candidates.len() {
                break;
            }
            let cand = &candidates[idx];
            () = line.push_str(cand);
            let pad = col_width.saturating_sub(str_display_width(cand));
            () = line.extend(iter::repeat_n(' ', pad));
        }
        () = lines.push(line.trim_end().to_string());
    }
    lines
}

/// Directory names in cwd matching `partial` (trailing `/` applied by `finalize_path_candidates`).
fn list_cwd_directories(partial: &str) -> Vec<String> {
    let entries = match fs::read_dir(".") {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut matches: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            name.starts_with(partial).then_some(name)
        })
        .collect();
    () = matches.sort();
    matches
}

/// List `.json` files in the current directory matching a partial prefix (yoyo-style).
fn list_json_files(partial: &str) -> Vec<String> {
    let entries = match fs::read_dir(".") {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut matches: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") && name.starts_with(partial) {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    () = matches.sort();
    matches
}

/// Append `/` to a directory only when it is the sole candidate at this level.
fn finalize_path_candidates(candidates: Vec<(String, bool)>) -> Vec<String> {
    let sole_dir = candidates.len() == 1 && candidates[0].1;
    candidates
        .into_iter()
        .map(|(name, is_dir)| {
            if sole_dir && is_dir {
                format!("{name}/")
            } else {
                name
            }
        })
        .collect()
}

/// Complete a partial file path by listing directory entries that match.
/// Directories get a trailing `/` only when they are the sole candidate at that level.
fn complete_file_path(partial: &str) -> Vec<String> {
    let path = Path::new(partial);

    let browse_directory = partial.ends_with('/') || partial.ends_with(std::path::MAIN_SEPARATOR);

    let (dir, mut file_prefix) = if browse_directory {
        (partial.to_string(), String::new())
    } else if let Some(parent) = path.parent() {
        let parent_str = if parent.as_os_str().is_empty() {
            String::from(".")
        } else {
            parent.to_string_lossy().to_string()
        };
        let file_prefix = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        (parent_str, file_prefix)
    } else {
        (String::from("."), String::from(partial))
    };

    if file_prefix.is_empty() && session_path_dotfile_browse(partial) {
        file_prefix = String::from(".");
    }

    if file_prefix.is_empty() && !browse_directory {
        return Vec::new();
    }

    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let dir_prefix = if session_path_dotfile_browse(partial) {
        if partial == "." {
            String::new()
        } else {
            partial.strip_suffix('.').unwrap_or(partial).to_string()
        }
    } else if dir == "." && !partial.contains('/') {
        String::new()
    } else if partial.ends_with('/') || partial.ends_with(MAIN_SEPARATOR) {
        partial.to_string()
    } else {
        let parent = path.parent().unwrap_or(Path::new(""));
        if parent.as_os_str().is_empty() {
            String::new()
        } else {
            format!("{}/", parent.display())
        }
    };

    let mut matches: Vec<(String, bool)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&file_prefix) {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let candidate = format!("{dir_prefix}{name}");
        matches.push((candidate, is_dir));
    }
    () = matches.sort_by(|a, b| a.0.cmp(&b.0));
    finalize_path_candidates(matches)
}

fn complete_session_paths(partial: &str) -> Vec<String> {
    if path_enters_subdirectory(partial) || path_like_partial(partial) {
        return complete_file_path(partial);
    }
    let mut matches: Vec<(String, bool)> = list_json_files(partial)
        .into_iter()
        .map(|name| (name, false))
        .collect();
    () = matches.extend(
        list_cwd_directories(partial)
            .into_iter()
            .map(|name| (name, true)),
    );
    () = matches.sort_by(|a, b| a.0.cmp(&b.0));
    () = matches.dedup_by(|a, b| a.0 == b.0);
    finalize_path_candidates(matches)
}

fn complete_args(
    command: &str,
    line: &str,
    prefix: &str,
    agent_config: &AgentConfig,
) -> Vec<String> {
    match command {
        "/provider" => complete_provider_names(prefix),
        "/model" => complete_model_line_args(line, prefix, agent_config),
        "/save" | "/load" => {
            if prefix.is_empty() {
                Vec::new()
            } else {
                complete_session_paths(prefix)
            }
        }
        "/help" => complete_commands(prefix),
        _ => Vec::new(),
    }
}

/// `/load` / `/save` path args: Tab extends typed characters only (no directory browsing).
pub fn session_path_char_completion(line: &str, _cursor: usize) -> bool {
    let (cmd, _) = command_name_and_args(line);
    matches!(cmd, "/load" | "/save") && line.contains(' ')
}

/// `/load ./` or `/load dir/` — Tab lists entries in that directory.
pub fn session_path_tab_lists_directory(line: &str, cursor: usize) -> bool {
    if !session_path_char_completion(line, cursor) {
        return false;
    }
    let prefix = token_prefix(line, cursor);
    !prefix.is_empty() && (prefix.ends_with('/') || prefix.ends_with(MAIN_SEPARATOR))
}

/// `/load ./.g` — filename component starts with `.` (dotfile char completion / list).
pub fn session_path_typed_dotfile_prefix(partial: &str) -> bool {
    if !path_like_partial(partial) || session_path_dotfile_browse(partial) {
        return false;
    }
    path_filename_component(partial).starts_with('.')
}

/// `/load ./.git/c` — completing inside a subdirectory (not cwd root).
pub fn session_path_inside_subdirectory(partial: &str) -> bool {
    if !path_like_partial(partial) {
        return false;
    }
    match Path::new(partial).parent() {
        None => false,
        Some(parent) => !parent.as_os_str().is_empty() && parent != Path::new("."),
    }
}

/// Bare `/load` / `/save` — Tab appends a space so the user can type a path (no directory scan).
pub fn session_command_ready_for_space_tab(line: &str, cursor: usize) -> bool {
    if cursor != line.chars().count() {
        return false;
    }
    let (cmd, args) = command_name_and_args(line);
    matches!(cmd, "/load" | "/save") && args.is_empty() && !line.contains(' ')
}

/// Append a space after bare `/load` / `/save` on Tab.
pub fn expand_session_command_tab(line: &mut String, cursor: &mut usize) -> bool {
    if !session_command_ready_for_space_tab(line, *cursor) {
        return false;
    }
    () = line.push(' ');
    *cursor = line.chars().count();
    true
}

/// `/load ./.` or `/load .` — Tab lists dot-prefixed entries (not char-only completion).
pub fn session_path_tab_shows_candidate_list(line: &str, cursor: usize) -> bool {
    if session_path_tab_lists_directory(line, cursor) {
        return true;
    }
    if !session_path_char_completion(line, cursor) {
        return false;
    }
    let prefix = token_prefix(line, cursor);
    path_like_partial(prefix) && session_path_dotfile_browse(prefix)
}

/// Gray ghost text shown after the cursor while typing slash commands (yoyo-style).
pub fn inline_hint(line: &str, cursor: usize, agent_config: &AgentConfig) -> Option<String> {
    let char_count = line.chars().count();
    if cursor != char_count {
        return None;
    }
    if !line.starts_with('/') {
        return None;
    }

    let typed = &line[1..];
    if typed.is_empty() {
        return None;
    }

    if typed.contains(' ') {
        let (cmd_part, arg_part) = typed.split_once(' ')?;
        if arg_part.is_empty() {
            return help_arg_hint(cmd_part).map(str::to_string);
        }
        let (cmd, _) = command_name_and_args(line);
        let prefix = token_prefix(line, cursor);
        return ghost_suffix(prefix, &complete_args(cmd, line, prefix, agent_config));
    }

    for cmd in KNOWN_COMMANDS {
        let cmd_name = &cmd.name[1..];
        if cmd_name.starts_with(typed) && cmd_name != typed {
            let rest = &cmd_name[typed.len()..];
            return Some(format!("{rest} — {}", cmd.summary));
        }
    }
    for cmd in KNOWN_COMMANDS {
        let cmd_name = &cmd.name[1..];
        if cmd_name == typed {
            return Some(format!(" — {}", cmd.summary));
        }
    }
    None
}

pub fn completions(line: &str, cursor: usize, agent_config: &AgentConfig) -> Vec<String> {
    if !line.starts_with('/') {
        return Vec::new();
    }

    let cursor = cursor.min(line.chars().count());
    let end_byte = line
        .char_indices()
        .nth(cursor)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    let prefix = token_prefix(line, cursor);

    let (cmd, _) = command_name_and_args(line);

    if line.get(..end_byte).is_some_and(|head| head.contains(' ')) {
        complete_args(cmd, line, prefix, agent_config)
    } else {
        complete_commands(prefix)
    }
}
