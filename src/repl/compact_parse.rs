#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactArg {
    Default,
    KeepRecent(usize),
    Preview,
    Invalid,
}

pub fn parse_compact_arg(arg: &str) -> CompactArg {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return CompactArg::Default;
    }
    if trimmed == "--preview" || trimmed == "preview" {
        return CompactArg::Preview;
    }
    if trimmed.eq_ignore_ascii_case("all") {
        return CompactArg::KeepRecent(2);
    }
    if let Ok(n) = trimmed.parse::<usize>() {
        return CompactArg::KeepRecent(n.max(2));
    }
    CompactArg::Invalid
}