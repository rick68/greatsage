//! Slash-command typo suggestions for unknown REPL commands.

use super::help_data::KNOWN_COMMANDS;

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate() {
        *val = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[a.len()][b.len()]
}

/// Suggest the closest known command for a mistyped slash command.
pub fn suggest_command(input: &str) -> Option<&'static str> {
    let cmd = input.split_whitespace().next().unwrap_or(input);

    if KNOWN_COMMANDS.iter().any(|c| c.name == cmd) {
        return None;
    }

    let prefix_matches: Vec<&str> = KNOWN_COMMANDS
        .iter()
        .map(|c| c.name)
        .filter(|known| known.starts_with(cmd))
        .collect();
    if prefix_matches.len() == 1 {
        return Some(prefix_matches[0]);
    }

    let mut best: Option<(&str, usize)> = None;
    for cmd_entry in KNOWN_COMMANDS {
        let dist = edit_distance(cmd, cmd_entry.name);
        match best {
            Some((_, best_dist)) if dist < best_dist => {
                best = Some((cmd_entry.name, dist));
            }
            None => best = Some((cmd_entry.name, dist)),
            _ => {}
        }
    }

    if let Some((suggestion, dist)) = best {
        // yoyo uses ≤2 for len≤5 when /fork exists; 3 covers short typos in our smaller catalog.
        let threshold = 3;
        if dist <= threshold {
            return Some(suggestion);
        }
    }

    None
}
