//! Project memory store — persistent notes in `.greatsage/memory.json`.

use {
    crate::config_paths::PROJECT_DIR,
    serde::{Deserialize, Serialize},
    std::{
        fs,
        path::{Path, PathBuf},
        process,
        time::{SystemTime, UNIX_EPOCH},
    },
};

const MEMORY_FILE: &str = "memory.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryEntry {
    pub note: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMemory {
    pub entries: Vec<MemoryEntry>,
}

pub fn memory_file_path(cwd: &Path) -> PathBuf {
    cwd.join(PROJECT_DIR).join(MEMORY_FILE)
}

pub fn load_memories_from(path: &Path) -> ProjectMemory {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ProjectMemory::default(),
    }
}

pub fn save_memories_to(memory: &ProjectMemory, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }
    let json =
        serde_json::to_string_pretty(memory).map_err(|e| format!("Serialization error: {e}"))?;
    fs::write(path, json).map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

pub fn add_memory(memory: &mut ProjectMemory, note: &str) {
    memory.entries.push(MemoryEntry {
        note: note.to_string(),
        timestamp: current_timestamp(),
    });
}

pub fn remove_memory(memory: &mut ProjectMemory, index: usize) -> Option<MemoryEntry> {
    if index < memory.entries.len() {
        Some(memory.entries.remove(index))
    } else {
        None
    }
}

pub fn search_memories<'a>(
    memory: &'a ProjectMemory,
    query: &str,
) -> Vec<(usize, &'a MemoryEntry)> {
    let query_lower = query.to_lowercase();
    memory
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.note.to_lowercase().contains(&query_lower))
        .collect()
}

pub fn format_memories_for_prompt(memory: &ProjectMemory) -> Option<String> {
    if memory.entries.is_empty() {
        return None;
    }
    let mut lines = vec!["## Project Memories".to_string(), String::new()];
    for entry in &memory.entries {
        lines.push(format!("- {} ({})", entry.note, entry.timestamp));
    }
    Some(lines.join("\n"))
}

/// List display: social-feed style timestamp (relative when recent, absolute when older).
pub fn format_memory_timestamp_display(iso_timestamp: &str) -> String {
    format_memory_timestamp_display_from(iso_timestamp, now_epoch_secs())
}

pub fn format_memory_timestamp_display_from(iso_timestamp: &str, now_secs: i64) -> String {
    let trimmed = iso_timestamp.trim();
    let Some(ts_parts) = parse_timestamp_parts(trimmed) else {
        return trimmed.to_string();
    };
    let ts_epoch = simple_local_epoch(ts_parts.0, ts_parts.1, ts_parts.2, ts_parts.3, ts_parts.4);
    if ts_epoch > now_secs {
        return trimmed.to_string();
    }

    let diff = (now_secs - ts_epoch) as u64;
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;

    if diff < MINUTE {
        return "just now".to_string();
    }
    if diff < HOUR {
        return format!("{}m ago", diff / MINUTE);
    }

    let now_parts = epoch_to_local_parts(now_secs);
    if same_calendar_day(ts_parts, now_parts) {
        return format!("{}h ago", diff / HOUR);
    }
    if is_yesterday(ts_parts, now_parts) {
        return format!("Yesterday at {:02}:{:02}", ts_parts.3, ts_parts.4);
    }
    if ts_parts.0 == now_parts.0 {
        return format!("{} {}", month_short(ts_parts.1), ts_parts.2);
    }
    format!("{} {}, {}", month_short(ts_parts.1), ts_parts.2, ts_parts.0)
}

type TimestampParts = (i32, u32, u32, u32, u32);

fn parse_timestamp_parts(s: &str) -> Option<TimestampParts> {
    if s.len() < 16 {
        return None;
    }
    let year: i32 = s.get(0..4)?.parse().ok()?;
    if s.as_bytes().get(4)? != &b'-' {
        return None;
    }
    let month: u32 = s.get(5..7)?.parse().ok()?;
    if s.as_bytes().get(7)? != &b'-' {
        return None;
    }
    let day: u32 = s.get(8..10)?.parse().ok()?;
    if s.as_bytes().get(10)? != &b' ' {
        return None;
    }
    let hour: u32 = s.get(11..13)?.parse().ok()?;
    if s.as_bytes().get(13)? != &b':' {
        return None;
    }
    let min: u32 = s.get(14..16)?.parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour >= 24 || min >= 60 {
        return None;
    }

    Some((year, month, day, hour, min))
}

fn parse_timestamp_to_epoch(s: &str) -> Option<i64> {
    let (year, month, day, hour, min) = parse_timestamp_parts(s)?;
    Some(simple_local_epoch(year, month, day, hour, min))
}

fn same_calendar_day(a: TimestampParts, b: TimestampParts) -> bool {
    a.0 == b.0 && a.1 == b.1 && a.2 == b.2
}

fn is_yesterday(ts: TimestampParts, now: TimestampParts) -> bool {
    let today_start = simple_local_epoch(now.0, now.1, now.2, 0, 0);
    let yesterday = epoch_to_local_parts(today_start - 86400);
    same_calendar_day(ts, yesterday)
}

fn month_short(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "???",
    }
}

fn epoch_to_local_parts(epoch: i64) -> TimestampParts {
    let mut days = epoch.div_euclid(86400);
    let day_secs = epoch.rem_euclid(86400);
    let hour = (day_secs / 3600) as u32;
    let min = ((day_secs % 3600) / 60) as u32;

    const MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    fn is_leap(y: i32) -> bool {
        (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
    }

    let mut year = 1970;
    loop {
        let year_days = if is_leap(year) { 366 } else { 365 };
        if days < year_days {
            break;
        }
        days -= year_days;
        year += 1;
    }

    for (idx, &md) in MONTH_DAYS.iter().enumerate() {
        let month_days = md + u32::from(idx == 1 && is_leap(year));
        if days < month_days as i64 {
            let month = (idx + 1) as u32;
            let day = days as u32 + 1;
            return (year, month, day, hour, min);
        }
        days -= month_days as i64;
    }

    (year, 12, 31, hour, min)
}

fn simple_local_epoch(year: i32, month: u32, day: u32, hour: u32, min: u32) -> i64 {
    const MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    fn is_leap(y: i32) -> bool {
        (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
    }

    let mut days: i64 = 0;
    if year >= 1970 {
        for y in 1970..year {
            days += if is_leap(y) { 366 } else { 365 };
        }
    } else {
        for y in year..1970 {
            days -= if is_leap(y) { 366 } else { 365 };
        }
    }

    for (m, &md) in MONTH_DAYS.iter().enumerate().take((month - 1) as usize) {
        days += md as i64;
        if m == 1 && is_leap(year) {
            days += 1;
        }
    }

    days += (day - 1) as i64;
    days * 86400 + hour as i64 * 3600 + min as i64 * 60
}

fn now_epoch_secs() -> i64 {
    process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8(o.stdout).ok()?;
                parse_timestamp_to_epoch(s.trim())
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        })
}

fn current_timestamp() -> String {
    process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn remember_output_lines(note: &str, cwd: &Path) -> Vec<String> {
    if note.is_empty() {
        return vec![
            "usage: /remember <note>".to_string(),
            "Save a project-specific memory that persists across sessions.".to_string(),
            "Examples:".to_string(),
            "  /remember this project uses sqlx for database access".to_string(),
            "  /remember tests require docker running".to_string(),
            "  /remember always run cargo fmt before committing".to_string(),
        ];
    }

    let path = memory_file_path(cwd);
    let mut memory = load_memories_from(&path);
    () = add_memory(&mut memory, note);
    match save_memories_to(&memory, &path) {
        Ok(()) => vec![format!(
            "✓ Remembered: \"{note}\" ({} total memories)",
            memory.entries.len()
        )],
        Err(e) => vec![format!("error saving memory: {e}")],
    }
}

pub fn memories_output_lines(query: &str, cwd: &Path) -> Vec<String> {
    let memory = load_memories_from(&memory_file_path(cwd));
    if memory.entries.is_empty() {
        return vec![
            "No project memories yet.".to_string(),
            "Use /remember <note> to add one.".to_string(),
        ];
    }

    if query.is_empty() {
        let mut lines = vec![format!("Project memories ({}):", memory.entries.len())];
        for (i, entry) in memory.entries.iter().enumerate() {
            lines.push(format!(
                "  [{i}] {} ({})",
                entry.note,
                format_memory_timestamp_display(&entry.timestamp)
            ));
        }
        () = lines.push("Use /forget <n> to remove a memory.".to_string());
        return lines;
    }

    let results = search_memories(&memory, query);
    if results.is_empty() {
        return vec![format!("No memories matching '{query}'.")];
    }

    let mut lines = vec![format!(
        "Found {} {} matching '{query}':",
        results.len(),
        if results.len() == 1 {
            "memory"
        } else {
            "memories"
        }
    )];
    for (i, entry) in &results {
        lines.push(format!(
            "  [{i}] {} ({})",
            entry.note,
            format_memory_timestamp_display(&entry.timestamp)
        ));
    }
    () = lines.push("Use /forget <n> to remove a memory.".to_string());
    lines
}

pub fn forget_output_lines(arg: &str, cwd: &Path) -> Vec<String> {
    if arg.is_empty() {
        return vec![
            "usage: /forget <n>".to_string(),
            "Remove a project memory by index. Use /memories to see indexes.".to_string(),
        ];
    }

    let index = match arg.parse::<usize>() {
        Ok(i) => i,
        Err(_) => {
            return vec![format!(
                "error: '{arg}' is not a valid index. Use /memories to see indexes."
            )];
        }
    };

    let path = memory_file_path(cwd);
    let mut memory = load_memories_from(&path);
    match remove_memory(&mut memory, index) {
        Some(removed) => match save_memories_to(&memory, &path) {
            Ok(()) => vec![format!(
                "✓ Forgot: \"{}\" ({} memories remaining)",
                removed.note,
                memory.entries.len()
            )],
            Err(e) => vec![format!("error saving memory: {e}")],
        },
        None => vec![format!(
            "error: index {index} out of range (have {} memories). Use /memories to see indexes.",
            memory.entries.len()
        )],
    }
}
