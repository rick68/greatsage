//! Session ECS → scrollback view model (pure helpers + systems).
//!
//! Operator `/help` / shell output lives in `TuiState::operator_panel`, **not** here.

use {
    crate::session::{ContentBlock, ContentBlockEntity},
    bevy::ecs::{
        query::With,
        resource::Resource,
        system::{Query, ResMut},
    },
};

/// One renderable scrollback line (conversation only).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScrollbackLine {
    pub text: String,
    /// Turn this line belongs to.
    pub turn_seq: Option<u64>,
    /// True for the `── turn N ──` boundary line (user-turn jump targets).
    pub is_turn_start: bool,
}

/// View model rebuilt from Session ECS for the focused session.
#[derive(Clone, Debug, Default, Resource)]
pub struct ScrollbackView {
    pub lines: Vec<ScrollbackLine>,
    /// Indices into `lines` where `is_turn_start` is true (ascending).
    pub turn_starts: Vec<usize>,
    pub empty_placeholder: bool,
}

impl ScrollbackView {
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// Build ordered lines from content blocks (sorted by turn_seq, then block_index, then seq).
pub fn lines_from_blocks(mut blocks: Vec<&ContentBlock>) -> Vec<ScrollbackLine> {
    () =blocks.sort_by(|a, b| {
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
                turn_seq: Some(block.turn_seq),
                is_turn_start: true,
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
        () = lines.push(ScrollbackLine {
            text,
            turn_seq: Some(block.turn_seq),
            is_turn_start: false,
        });
    }
    lines
}

pub fn turn_starts_from_lines(lines: &[ScrollbackLine]) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| l.is_turn_start.then_some(i))
        .collect()
}

pub fn rebuild_scrollback_view(
    blocks_q: Query<&ContentBlock, With<ContentBlockEntity>>,
    mut view: ResMut<ScrollbackView>,
) {
    let refs: Vec<&ContentBlock> = blocks_q.iter().collect();
    if refs.is_empty() {
        view.lines.clear();
        view.turn_starts.clear();
        view.empty_placeholder = true;
        return;
    }
    view.lines = lines_from_blocks(refs);
    view.turn_starts = turn_starts_from_lines(&view.lines);
    view.empty_placeholder = false;
}
