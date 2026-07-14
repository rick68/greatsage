//! Session ECS → scrollback view model (pure helpers + systems).

use {
    crate::session::{ContentBlock, ContentBlockEntity},
    bevy::ecs::{
        query::With,
        resource::Resource,
        system::{Query, ResMut},
    },
};

/// One renderable scrollback line (foundation: plain text).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScrollbackLine {
    pub text: String,
}

/// View model rebuilt from Session ECS for the focused session.
#[derive(Clone, Debug, Default, Resource)]
pub struct ScrollbackView {
    pub lines: Vec<ScrollbackLine>,
    pub empty_placeholder: bool,
}

/// Build ordered lines from content blocks (sorted by turn_seq, then block_index, then seq).
pub fn lines_from_blocks(mut blocks: Vec<&ContentBlock>) -> Vec<ScrollbackLine> {
    blocks.sort_by(|a, b| {
        a.turn_seq
            .cmp(&b.turn_seq)
            .then(a.block_index.cmp(&b.block_index))
            .then(a.seq.cmp(&b.seq))
    });

    let mut lines = Vec::new();
    let mut last_turn: Option<u64> = None;
    for block in blocks {
        if last_turn != Some(block.turn_seq) {
            lines.push(ScrollbackLine {
                text: format!("── turn {} ──", block.turn_seq),
            });
            last_turn = Some(block.turn_seq);
        }
        let kind = if block.content_kind.is_empty() {
            "block"
        } else {
            block.content_kind.as_str()
        };
        let body = block.content_text.trim();
        let text = if body.is_empty() {
            format!("[{kind}]")
        } else {
            format!("[{kind}] {body}")
        };
        () = lines.push(ScrollbackLine { text });
    }
    lines
}

pub fn rebuild_scrollback_view(
    blocks_q: Query<&ContentBlock, With<ContentBlockEntity>>,
    mut view: ResMut<ScrollbackView>,
) {
    let refs: Vec<&ContentBlock> = blocks_q.iter().collect();
    if refs.is_empty() {
        view.lines.clear();
        view.empty_placeholder = true;
        return;
    }
    view.lines = lines_from_blocks(refs);
    view.empty_placeholder = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(turn_seq: u64, block_index: u32, seq: u64, kind: &str, text: &str) -> ContentBlock {
        ContentBlock {
            seq,
            turn_seq,
            block_index,
            recorded_at_ms: 0,
            source_timestamp_ms: 0,
            provenance_kind: String::new(),
            content_kind: kind.to_string(),
            content_text: text.to_string(),
            content_json: String::new(),
            tool_call_id: None,
            tool_name: None,
            is_error: None,
        }
    }

    #[test]
    fn lines_ordered_by_turn_then_block_index() {
        let a = block(2, 0, 10, "text", "second turn");
        let b = block(1, 1, 2, "text", "first turn b");
        let c = block(1, 0, 1, "text", "first turn a");
        let lines = lines_from_blocks(vec![&a, &b, &c]);
        assert_eq!(lines[0].text, "── turn 1 ──");
        assert!(lines[1].text.contains("first turn a"));
        assert!(lines[2].text.contains("first turn b"));
        assert_eq!(lines[3].text, "── turn 2 ──");
        assert!(lines[4].text.contains("second turn"));
    }

    #[test]
    fn empty_blocks_yield_empty_lines() {
        assert!(lines_from_blocks(vec![]).is_empty());
    }
}
