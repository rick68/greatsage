//! Terminal display-width helpers for TUI prompt / truncation (wide CJK, etc.).

use unicode_width::UnicodeWidthChar;

/// Display columns for a single char (0 for combining, 1 or 2 for most glyphs).
pub fn char_display_width(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(0)
}

/// Display columns for a string (sum of char widths).
pub fn str_display_width(s: &str) -> usize {
    s.chars().map(char_display_width).sum()
}

/// Display width of the caret covering the glyph at the insertion point.
///
/// - End of draft → 1 column (insert bar).
/// - Next char is wide (CJK, …) → that char's display width (usually 2).
pub fn caret_width_for_after(after: &str) -> usize {
    match after.chars().next() {
        Some(c) => char_display_width(c).max(1),
        None => 1,
    }
}

/// Truncate `s` so its display width is ≤ `max_cols` (does not split a wide char).
pub fn truncate_to_width(s: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut cols = 0usize;
    for ch in s.chars() {
        let w = char_display_width(ch);
        if w == 0 {
            // Keep combining marks attached when we already have a base.
            if !out.is_empty() {
                () = out.push(ch);
            }
            continue;
        }
        if cols + w > max_cols {
            break;
        }
        () = out.push(ch);
        cols += w;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_width_one_per_char() {
        assert_eq!(str_display_width("hello"), 5);
        assert_eq!(str_display_width("> "), 2);
    }

    #[test]
    fn cjk_is_two_cols() {
        assert_eq!(char_display_width('中'), 2);
        assert_eq!(str_display_width("中文"), 4);
        assert_eq!(str_display_width("/測"), 3); // '/'=1 + 測=2
    }

    #[test]
    fn caret_offset_after_wide_prefix() {
        // "> " + "你好" → 2 + 4 = 6 columns before caret
        let prefix = "> ";
        let before = "你好";
        assert_eq!(
            str_display_width(prefix) + str_display_width(before),
            6
        );
    }

    #[test]
    fn caret_width_matches_glyph_under_cursor() {
        // End of line → 1; on CJK → 2; on ASCII → 1
        assert_eq!(caret_width_for_after(""), 1);
        assert_eq!(caret_width_for_after("中文"), 2);
        assert_eq!(caret_width_for_after("ab"), 1);
    }

    #[test]
    fn truncate_does_not_split_wide_char() {
        let s = "ab中cd";
        // max 3: 'a','b' (2) + '中' needs 2 more → stop at "ab"
        assert_eq!(truncate_to_width(s, 3), "ab");
        assert_eq!(truncate_to_width(s, 4), "ab中");
        assert_eq!(truncate_to_width(s, 5), "ab中c");
    }
}
