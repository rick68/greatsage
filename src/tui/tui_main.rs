#![allow(
    dead_code,
    unused,
    clippy::collapsible_if,
    clippy::single_match,
    clippy::needless_borrow
)]
//! # TUI Main — Output Rendering Architecture
//!
//! ## OutputBlock model
//! Output is stored as `Vec<OutputBlock>` instead of a flat `Vec<Line>`.
//! `rendered_flat_lines()` flattens blocks to `Vec<Line<'static>>`, which
//! `hard_wrap_output_lines()` then word-wraps to the terminal width.
//!
//! ## Coordinate mapping invariant
//! Every wrap calculation uses `unicode_width` — never `.len()` — so that
//! terminal column positions stay consistent with scrollbar math and any
//! future LineMap for mouse-to-text coordinate translation.
//!
//! ## ThinkingBlock lifecycle
//! `begin_thinking()` → `append_thinking(delta)` × N → `end_thinking(tokens)`
//! Streaming: spinner + live elapsed.  Done: ▶/▼ + elapsed + token count.
//!
//! ### ThinkingBlock keyboard controls (work from any focus area)
//! - `t` — toggle the **most recent** completed ThinkingBlock
//! - `a` — collapse **all** completed ThinkingBlocks
//! - `A` — expand  **all** completed ThinkingBlocks
//!
//! `a`/`A` are the only way to control ThinkingBlocks other than the last
//! one, since there is currently no per-block cursor navigation.
//!
//! ## Mouse text selection
//! Mouse capture handles scroll-wheel and future click-to-toggle.
//! Terminal-native selection is available via Shift+drag (terminal emulator
//! layer; no app code needed).  Full in-app selection requires a LineMap
//! built alongside `rendered_flat_lines` — deferred to a future milestone.

use {
    super::RenderNeeded,
    crate::agents::{CodingAgentPromptChannel, CodingAgentTotalTokenUsage},
    bevy::{
        app::{App, AppExit, PreUpdate, Update},
        ecs::{
            change_detection::{NonSendMut, Res, ResMut},
            message::{MessageReader, MessageWriter},
            schedule::IntoScheduleConfigs,
            system::Local,
        },
        state::{
            app::AppExtStates,
            condition::in_state,
            state::{NextState, State, States},
        },
        time::{Time, Timer, TimerMode},
    },
    bevy_ratatui::{
        RatatuiContext,
        crossterm::{self, cursor::SetCursorStyle, execute},
        event::{KeyMessage, MouseMessage},
    },
    ratatui::{
        Frame,
        layout::{Constraint, Layout, Rect},
        style::{Style, Stylize},
        text::{Line, Span},
        widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    },
    std::{
        io::stdout,
        iter::{DoubleEndedIterator, ExactSizeIterator, Iterator},
        time::{Duration, Instant},
    },
    strum::{EnumCount, FromRepr},
    unicode_width::{UnicodeWidthChar, UnicodeWidthStr},
};

const CURSOR_BLINK_INTERVAL_MS: u64 = 530;

/// What happens when the user left-clicks a row in the output area.
#[derive(Clone, Copy, PartialEq)]
enum ClickAction {
    /// Clicking this row selects / deselects the owning `ResponseBlock`.
    Select,
    /// Clicking this row toggles `thinkings[idx]` inside the `ResponseBlock`.
    ToggleThinking(usize),
}
const PROMPT_PREFIX: &str = "🤖 > ";
const SPINNER: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

// Border colours — edit here to restyle all panels at once.
const COLOR_BORDER_FOCUSED: ratatui::style::Color = ratatui::style::Color::White;
const COLOR_BORDER_UNFOCUSED: ratatui::style::Color = ratatui::style::Color::DarkGray;

#[derive(Clone, Copy, Debug, Default, EnumCount, Eq, FromRepr, Hash, PartialEq, States)]
#[repr(u8)]
pub enum TuiMainFocus {
    #[default]
    InputArea,
    OutputArea,
}

impl Iterator for TuiMainFocus {
    type Item = TuiMainFocus;

    fn next(&mut self) -> Option<Self::Item> {
        let next = ((*self as usize + 1) % Self::COUNT) as u8;
        TuiMainFocus::from_repr(next).inspect(|item| *self = *item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (Self::COUNT, Some(Self::COUNT))
    }
}

impl ExactSizeIterator for TuiMainFocus {}

impl DoubleEndedIterator for TuiMainFocus {
    fn next_back(&mut self) -> Option<Self::Item> {
        let next = if (*self as u8) == 0 {
            (Self::COUNT - 1) as u8
        } else {
            (*self as u8) - 1
        };
        TuiMainFocus::from_repr(next).inspect(|item| *self = *item)
    }
}

/// A live or completed reasoning block emitted by the LLM's extended thinking.
///
/// Lifecycle: `begin_thinking()` → `append_thinking(delta)` × N → `end_thinking(tokens)`.
/// While `streaming` is true the header shows a spinner and live elapsed time.
/// After `end_thinking` the header shows elapsed + token count and the body
/// can be toggled with `t` (OutputArea focus).
pub struct ThinkingBlock {
    raw: String,
    lines: Vec<Line<'static>>,
    elapsed_secs: f32,
    token_count: u32,
    expanded: bool,
    streaming: bool,
    start_instant: Instant,
}

impl ThinkingBlock {
    fn new() -> Self {
        Self {
            raw: String::new(),
            lines: Vec::new(),
            elapsed_secs: 0.0,
            token_count: 0,
            expanded: false,
            streaming: true,
            start_instant: Instant::now(),
        }
    }

    fn spinner_frame(&self) -> &'static str {
        let ms = self.start_instant.elapsed().as_millis() as usize;
        SPINNER[(ms / 100) % SPINNER.len()]
    }

    fn render_header(&self) -> Line<'static> {
        if self.streaming {
            Line::from(vec![
                Span::from("💭 Thinking  "),
                Span::from(self.spinner_frame()).yellow(),
                Span::from(format!(
                    "  {:.1}s",
                    self.start_instant.elapsed().as_secs_f32()
                ))
                .dim(),
            ])
        } else {
            let arrow = if self.expanded { "▼" } else { "▶" };
            // Show "t" hint only when collapsed so user knows content is there.
            let hint = if self.expanded { "" } else { "  (t)" };
            Line::from(vec![
                Span::from(format!("💭 Thinking  {arrow}  ")),
                Span::from(format!(
                    "[{:.1}s · {} tokens]",
                    self.elapsed_secs, self.token_count
                ))
                .dim(),
                Span::from(hint).dark_gray(),
            ])
        }
    }
}

/// A single tool invocation within a [`ResponseBlock`].
struct ToolCallEntry {
    summary: String,
    running: bool,
    is_error: bool,
    error_snippet: String,
    start_instant: Instant,
}

impl ToolCallEntry {
    fn render(&self) -> Line<'static> {
        let elapsed = self.start_instant.elapsed().as_secs_f32();
        let elapsed_str = format!("{elapsed:.1}s");
        if self.running {
            let frame =
                SPINNER[(self.start_instant.elapsed().as_millis() as usize / 100) % SPINNER.len()];
            Line::from(vec![
                Span::from(self.summary.clone()).yellow(),
                Span::from(format!("  {frame}  {elapsed_str}"))
                    .yellow()
                    .dim(),
            ])
        } else if self.is_error {
            let mut spans = vec![
                Span::from(self.summary.clone()).red(),
                Span::from(format!("  ❌  {elapsed_str}")).red(),
            ];
            if !self.error_snippet.is_empty() {
                spans.push(Span::from(format!("  {}", self.error_snippet)).red().dim());
            }
            Line::from(spans)
        } else {
            Line::from(vec![
                Span::from(self.summary.clone()),
                Span::from(format!("  ✅  {elapsed_str}")).dim(),
            ])
        }
    }
}

/// One complete (or in-progress) agent response turn.
///
/// Groups the extended-thinking block(s), all tool calls made during the turn,
/// and the final text response into a single visual unit.
///
/// Click with the mouse to **select** a block; while selected, pressing `t`
/// toggles the ThinkingBlock inside that specific turn.  `a`/`A` operate on
/// all turns at once.
pub struct ResponseBlock {
    thinkings: Vec<ThinkingBlock>,
    tool_calls: Vec<ToolCallEntry>,
    /// Raw accumulated text; kept for future markdown re-render passes.
    #[allow(dead_code)]
    text_raw: String,
    text_lines: Vec<Line<'static>>,
    text_streaming: bool,
    sealed: bool,
}

impl ResponseBlock {
    fn new() -> Self {
        Self {
            thinkings: Vec::new(),
            tool_calls: Vec::new(),
            text_raw: String::new(),
            text_lines: Vec::new(),
            text_streaming: false,
            sealed: false,
        }
    }

    fn has_spinner(&self) -> bool {
        self.thinkings.iter().any(|tb| tb.streaming) || self.tool_calls.iter().any(|tc| tc.running)
    }

    /// Render all content lines for this turn.
    ///
    /// Returns `(lines, actions)` where every entry in `actions` corresponds
    /// to the same-index entry in `lines` and describes what a left-click on
    /// that row should do:
    /// - `ClickAction::ToggleThinking(i)` — the row is the `i`-th thinking
    ///   header; clicking it toggles `thinkings[i]`.
    /// - `ClickAction::Select` — any other row; clicking selects the block.
    ///
    /// When `selected` is true every content line receives a cyan `▌ ` gutter
    /// and the bottom rule becomes cyan.
    fn render_lines(&self, selected: bool) -> (Vec<Line<'static>>, Vec<ClickAction>) {
        let mut content: Vec<Line<'static>> = Vec::new();
        let mut actions: Vec<ClickAction> = Vec::new();

        // Thinking blocks — header row gets ToggleThinking, body rows get Select.
        for (ti, tb) in self.thinkings.iter().enumerate() {
            content.push(tb.render_header());
            actions.push(ClickAction::ToggleThinking(ti));
            if tb.streaming || tb.expanded {
                for line in &tb.lines {
                    let mut spans = vec![Span::from("  │ ").dim()];
                    spans.extend(line.spans.iter().cloned());
                    content.push(Line::from(spans));
                    actions.push(ClickAction::Select);
                }
                if !tb.streaming {
                    content.push(Line::from(Span::from("  └─").dim()));
                    actions.push(ClickAction::Select);
                }
            }
        }

        // Tool calls
        for tc in &self.tool_calls {
            content.push(tc.render());
            actions.push(ClickAction::Select);
        }

        // Text — blank separator before text when other content precedes it
        if !self.text_lines.is_empty() || self.text_streaming {
            if !self.thinkings.is_empty() || !self.tool_calls.is_empty() {
                content.push(Line::from(""));
                actions.push(ClickAction::Select);
            }
            for line in &self.text_lines {
                content.push(line.clone());
                actions.push(ClickAction::Select);
            }
        }

        // Bottom rule (always Select)
        if selected {
            let lines: Vec<Line<'static>> = content
                .into_iter()
                .map(|line| {
                    let mut spans = vec![Span::from("▌ ").light_blue()];
                    spans.extend(line.spans);
                    Line::from(spans)
                })
                .collect();
            actions.push(ClickAction::Select);
            let mut lines = lines;
            lines.push(Line::from(
                Span::from("▌─────────────────────────────────────────────────").light_blue(),
            ));
            (lines, actions)
        } else {
            actions.push(ClickAction::Select);
            let mut lines = content;
            lines.push(divider_line());
            (lines, actions)
        }
    }
}

/// A structural unit of TUI output.
pub enum OutputBlock {
    /// Static lines: status messages, startup info, slash command output.
    Lines(Vec<Line<'static>>),
    /// One complete (or in-progress) agent response turn.
    Response(ResponseBlock),
}

fn divider_line() -> Line<'static> {
    Line::from(Span::from("─".repeat(50)).dark_gray())
}

#[derive(Default)]
pub struct TuiMain {
    input: String,
    byte_index: usize, // byte offset of cursor in input
    history: Vec<String>,
    history_pos: Option<usize>, // None = editing new input, Some(i) = viewing history[i]
    history_draft: String,      // saved draft when navigating history
    pub blocks: Vec<OutputBlock>,
    output_area: Rect,
    show_cursor: bool,
    focused: TuiMainFocus,
    vertical_scroll: usize,
    vertical_scroll_state: ScrollbarState,
    /// Maps each post-wrap output row to `(block_index, ClickAction)`.
    /// `None` for rows that belong to a `Lines` block.  Updated every frame in `draw`.
    line_map: Vec<Option<(usize, ClickAction)>>,
    /// Index of the currently selected `Response` block (`None` = nothing selected).
    /// Set by mouse click; used by `t` to target the right ThinkingBlock.
    selected_block: Option<usize>,
}

impl TuiMain {
    #[cfg(test)]
    pub(crate) fn set_input_public(&mut self, s: String) {
        self.set_input(s);
    }
    #[cfg(test)]
    pub(crate) fn push_history_public(&mut self, s: &str) {
        self.push_history(s);
    }
    #[cfg(test)]
    pub(crate) fn clear_input_public(&mut self) {
        self.clear_input();
    }
    #[cfg(test)]
    pub(crate) fn input_is_empty(&self) -> bool {
        self.input.is_empty()
    }
    #[cfg(test)]
    pub(crate) fn last_history(&self) -> Option<&str> {
        self.history.last().map(|s| s.as_str())
    }

    // ── Output public API ────────────────────────────────────────────────────

    /// Append a single line to the output, coalescing into the last Lines block.
    pub fn push_line(&mut self, line: Line<'static>) {
        match self.blocks.last_mut() {
            Some(OutputBlock::Lines(lines)) => lines.push(line),
            _ => self.blocks.push(OutputBlock::Lines(vec![line])),
        }
    }

    /// Append multiple lines to the output.
    pub fn push_lines(&mut self, new_lines: Vec<Line<'static>>) {
        match self.blocks.last_mut() {
            Some(OutputBlock::Lines(lines)) => lines.extend(new_lines),
            _ => self.blocks.push(OutputBlock::Lines(new_lines)),
        }
    }

    /// Replace the last line in the most recent Lines block.
    #[allow(dead_code)]
    pub fn replace_last_line(&mut self, line: Line<'static>) {
        for block in self.blocks.iter_mut().rev() {
            if let OutputBlock::Lines(lines) = block
                && let Some(last) = lines.last_mut()
            {
                *last = line;
                return;
            }
        }
    }

    // ── Response turn API ────────────────────────────────────────────────────

    /// Return a mutable reference to the most recent `Response` block.
    fn current_response_mut(&mut self) -> Option<&mut ResponseBlock> {
        self.blocks.iter_mut().rev().find_map(|b| {
            if let OutputBlock::Response(r) = b {
                Some(r)
            } else {
                None
            }
        })
    }

    /// Open a new agent response turn.  Must be called before any
    /// `begin_thinking` / `begin_tool_call` / `begin_streaming_text` for that turn.
    pub fn begin_response(&mut self) {
        self.blocks
            .push(OutputBlock::Response(ResponseBlock::new()));
    }

    /// Seal the current response turn once all streaming has finished.
    pub fn end_response(&mut self) {
        if let Some(resp) = self.current_response_mut() {
            resp.sealed = true;
        }
    }

    /// Open a new ThinkingBlock inside the current response turn.
    pub fn begin_thinking(&mut self) {
        if let Some(resp) = self.current_response_mut() {
            resp.thinkings.push(ThinkingBlock::new());
        }
    }

    /// Append a thinking delta to the most recent streaming ThinkingBlock.
    pub fn append_thinking(&mut self, raw: &str) {
        if let Some(resp) = self.current_response_mut() {
            if let Some(tb) = resp.thinkings.iter_mut().rev().find(|tb| tb.streaming) {
                tb.raw.push_str(raw);
                tb.lines = tb
                    .raw
                    .lines()
                    .map(|l| Line::from(Span::from(l.to_string()).dim()))
                    .collect();
            }
        }
    }

    /// Seal the most recent streaming ThinkingBlock with its elapsed time and token count.
    pub fn end_thinking(&mut self, token_count: u32) {
        if let Some(resp) = self.current_response_mut() {
            if let Some(tb) = resp.thinkings.iter_mut().rev().find(|tb| tb.streaming) {
                tb.elapsed_secs = tb.start_instant.elapsed().as_secs_f32();
                tb.token_count = token_count;
                tb.streaming = false;
            }
        }
    }

    /// Open a new ToolCall inside the current response turn.
    pub fn begin_tool_call(&mut self, summary: String) {
        if let Some(resp) = self.current_response_mut() {
            resp.tool_calls.push(ToolCallEntry {
                summary,
                running: true,
                is_error: false,
                error_snippet: String::new(),
                start_instant: Instant::now(),
            });
        }
    }

    /// Seal the most recent running ToolCall with its outcome.
    pub fn finish_tool_call(&mut self, is_error: bool, error_snippet: String) {
        if let Some(resp) = self.current_response_mut() {
            if let Some(tc) = resp.tool_calls.iter_mut().rev().find(|tc| tc.running) {
                tc.running = false;
                tc.is_error = is_error;
                tc.error_snippet = error_snippet;
            }
        }
    }

    /// Mark the current response turn's text stream as started.
    pub fn begin_streaming_text(&mut self) {
        if let Some(resp) = self.current_response_mut() {
            resp.text_streaming = true;
        }
    }

    /// Replace the text lines in the current response turn (called on each delta).
    pub fn update_streaming_text(&mut self, new_lines: Vec<Line<'static>>) {
        if let Some(resp) = self.current_response_mut() {
            resp.text_lines = new_lines;
        }
    }

    /// Seal the text stream in the current response turn.
    pub fn finalize_streaming_text(&mut self) {
        if let Some(resp) = self.current_response_mut() {
            resp.text_streaming = false;
        }
    }

    // ── ThinkingBlock keyboard controls ─────────────────────────────────────

    /// Toggle the most recent completed ThinkingBlock.
    ///
    /// If a `Response` block is currently selected (`selected_block`), the
    /// toggle targets that specific block; otherwise it falls through to the
    /// last `Response` block in the history.
    fn toggle_last_thinking(&mut self) {
        let target = self.selected_block.or_else(|| {
            self.blocks
                .iter()
                .rposition(|b| matches!(b, OutputBlock::Response(_)))
        });
        if let Some(idx) = target {
            if let Some(OutputBlock::Response(resp)) = self.blocks.get_mut(idx) {
                for tb in resp.thinkings.iter_mut().rev() {
                    if !tb.streaming {
                        tb.expanded = !tb.expanded;
                        return;
                    }
                }
            }
        }
    }

    /// Expand all completed ThinkingBlocks across every response turn.
    fn expand_all_thinking(&mut self) {
        for block in self.blocks.iter_mut() {
            if let OutputBlock::Response(resp) = block {
                for tb in resp.thinkings.iter_mut() {
                    if !tb.streaming {
                        tb.expanded = true;
                    }
                }
            }
        }
    }

    /// Collapse all completed ThinkingBlocks across every response turn.
    fn collapse_all_thinking(&mut self) {
        for block in self.blocks.iter_mut() {
            if let OutputBlock::Response(resp) = block {
                for tb in resp.thinkings.iter_mut() {
                    if !tb.streaming {
                        tb.expanded = false;
                    }
                }
            }
        }
    }

    // ── Rendering helpers ────────────────────────────────────────────────────

    /// Flatten all blocks into lines for wrapping and display.
    ///
    /// Also returns a parallel LineMap of `Option<(block_index, ClickAction)>`.
    /// `None` = row belongs to a `Lines` block.  The LineMap is propagated
    /// through `hard_wrap_output_lines_with_map` so that after wrapping each
    /// terminal row still knows which block it belongs to and what a click does.
    fn rendered_flat_lines(&self) -> (Vec<Line<'static>>, Vec<Option<(usize, ClickAction)>>) {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut map: Vec<Option<(usize, ClickAction)>> = Vec::new();
        for (i, block) in self.blocks.iter().enumerate() {
            match block {
                OutputBlock::Lines(ls) => {
                    for line in ls {
                        lines.push(line.clone());
                        map.push(None);
                    }
                }
                OutputBlock::Response(resp) => {
                    let selected = self.selected_block == Some(i);
                    let (block_lines, block_actions) = resp.render_lines(selected);
                    for (line, action) in block_lines.into_iter().zip(block_actions) {
                        lines.push(line);
                        map.push(Some((i, action)));
                    }
                }
            }
        }
        (lines, map)
    }

    /// Word-wrap `lines` to `inner_width` columns, propagating a parallel
    /// `map` so that every wrapped row retains its source block index and
    /// click action.
    fn hard_wrap_output_lines_with_map(
        lines: &[Line<'_>],
        map: &[Option<(usize, ClickAction)>],
        inner_width: usize,
    ) -> (Vec<Line<'static>>, Vec<Option<(usize, ClickAction)>>) {
        if inner_width == 0 {
            return (
                lines.iter().map(|l| Line::from(l.to_string())).collect(),
                map.to_vec(),
            );
        }
        let mut result: Vec<Line<'static>> = Vec::new();
        let mut result_map: Vec<Option<(usize, ClickAction)>> = Vec::new();
        for (line, entry) in lines.iter().zip(map.iter()) {
            let block_idx = *entry;
            let mut current_spans: Vec<Span<'static>> = Vec::new();
            let mut current_width = 0usize;
            for span in &line.spans {
                let style = span.style;
                let mut buf = String::new();
                for ch in span.content.chars() {
                    let ch_w = ch.width().unwrap_or(1);
                    if current_width + ch_w > inner_width && current_width > 0 {
                        if !buf.is_empty() {
                            current_spans.push(Span::styled(std::mem::take(&mut buf), style));
                        }
                        result.push(Line::from(std::mem::take(&mut current_spans)));
                        result_map.push(block_idx);
                        current_width = 0;
                    }
                    buf.push(ch);
                    current_width += ch_w;
                }
                if !buf.is_empty() {
                    current_spans.push(Span::styled(buf, style));
                }
            }
            result.push(Line::from(current_spans));
            result_map.push(block_idx);
        }
        (result, result_map)
    }

    fn display_index(&self) -> usize {
        self.input[..self.byte_index].width()
    }

    fn input_display_lines(&self, inner_width: usize) -> Vec<String> {
        let display_text = format!("{PROMPT_PREFIX}{}", self.input);
        if inner_width == 0 {
            return vec![display_text];
        }
        let mut lines: Vec<String> = Vec::new();
        let mut current_line = String::new();
        let mut current_width = 0usize;
        for ch in display_text.chars() {
            let ch_w = ch.width().unwrap_or(1);
            if current_width + ch_w > inner_width && !current_line.is_empty() {
                lines.push(std::mem::take(&mut current_line));
                current_width = 0;
            }
            current_line.push(ch);
            current_width += ch_w;
        }
        lines.push(current_line);
        lines
    }

    fn insert_char(&mut self, c: char) {
        self.input.insert(self.byte_index, c);
        self.byte_index += c.len_utf8();
    }

    fn delete_before(&mut self) {
        if self.byte_index == 0 {
            return;
        }
        let c = self.input[..self.byte_index].chars().next_back().unwrap();
        self.byte_index -= c.len_utf8();
        self.input.remove(self.byte_index);
    }

    fn delete_after(&mut self) {
        if self.byte_index < self.input.len() {
            self.input.remove(self.byte_index);
        }
    }

    fn cursor_left(&mut self) {
        if let Some(c) = self.input[..self.byte_index].chars().next_back() {
            self.byte_index -= c.len_utf8();
        }
    }

    fn cursor_right(&mut self) {
        if let Some(c) = self.input[self.byte_index..].chars().next() {
            self.byte_index += c.len_utf8();
        }
    }

    fn cursor_to_start(&mut self) {
        self.byte_index = 0;
    }

    fn cursor_to_end(&mut self) {
        self.byte_index = self.input.len();
    }

    fn set_input(&mut self, s: String) {
        self.input = s;
        self.cursor_to_end();
    }

    fn clear_input(&mut self) {
        self.input.clear();
        self.byte_index = 0;
    }

    /// Clears the REPL output buffer and resets scrolling.
    fn clear_output(&mut self) {
        self.blocks.clear();
        self.vertical_scroll = 0;
        self.vertical_scroll_state = ScrollbarState::default();
    }

    fn push_history(&mut self, s: &str) {
        if !s.is_empty() && self.history.last().map(|l| l != s).unwrap_or(true) {
            self.history.push(s.to_owned());
        }
        self.history_pos = None;
        self.history_draft.clear();
    }

    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let new_pos = match self.history_pos {
            None => {
                self.history_draft = self.input.clone();
                self.history.len() - 1
            }
            Some(0) => return,
            Some(i) => i - 1,
        };
        self.history_pos = Some(new_pos);
        let entry = self.history[new_pos].clone();
        self.set_input(entry);
    }

    fn history_next(&mut self) {
        match self.history_pos {
            None => (),
            Some(i) if i + 1 >= self.history.len() => {
                self.history_pos = None;
                let draft = self.history_draft.clone();
                self.set_input(draft);
            }
            Some(i) => {
                self.history_pos = Some(i + 1);
                let entry = self.history[i + 1].clone();
                self.set_input(entry);
            }
        }
    }

    fn output_area_height(&self) -> usize {
        const BORDER: u16 = 2;
        self.output_area.height.saturating_sub(BORDER) as usize
    }

    fn total_visual_rows(&self) -> usize {
        let inner_width = self.output_area.width.saturating_sub(2) as usize;
        let (flat, _) = self.rendered_flat_lines();
        if inner_width == 0 {
            return flat.len();
        }
        let mut count = 0usize;
        for line in &flat {
            let mut current_width = 0usize;
            for span in &line.spans {
                for ch in span.content.chars() {
                    let ch_w = ch.width().unwrap_or(1);
                    if current_width + ch_w > inner_width && current_width > 0 {
                        count += 1;
                        current_width = 0;
                    }
                    current_width += ch_w;
                }
            }
            count += 1;
        }
        count
    }

    fn max_scroll(&self) -> usize {
        self.total_visual_rows()
            .saturating_sub(self.output_area_height())
    }

    fn draw(&mut self, frame: &mut Frame, token_usage: Option<&CodingAgentTotalTokenUsage>) {
        let area = frame.area();
        let inner_width = area.width.saturating_sub(2) as usize;
        let input_lines = self.input_display_lines(inner_width);
        let input_height = (input_lines.len() as u16 + 2).max(3);

        let vertical = Layout::vertical([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(input_height),
        ]);
        let [output_area, status_area, input_area] = vertical.areas(area);
        self.output_area = output_area;

        let output_inner_width = output_area.width.saturating_sub(2) as usize;
        let (flat, flat_map) = self.rendered_flat_lines();
        let (wrapped, wrapped_map) =
            Self::hard_wrap_output_lines_with_map(&flat, &flat_map, output_inner_width);
        self.line_map = wrapped_map;
        let total_rows = wrapped.len();
        let new_max = total_rows.saturating_sub(self.output_area_height());
        self.vertical_scroll = self.vertical_scroll.min(new_max);
        let output_border_color = if self.focused == TuiMainFocus::OutputArea {
            COLOR_BORDER_FOCUSED
        } else {
            COLOR_BORDER_UNFOCUSED
        };
        let output = Paragraph::new(wrapped)
            .style(Style::default())
            .block(
                Block::bordered()
                    .title("Output")
                    .border_style(Style::default().fg(output_border_color)),
            )
            .scroll((self.vertical_scroll as u16, 0));
        let scroll_positions = total_rows.saturating_sub(self.output_area_height()) + 1;
        self.vertical_scroll_state = self
            .vertical_scroll_state
            .content_length(scroll_positions)
            .viewport_content_length(self.output_area_height())
            .position(self.vertical_scroll);

        () = frame.render_widget(output, output_area);
        () = frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            output_area,
            &mut self.vertical_scroll_state,
        );

        // Draw status bar with token usage
        let status_text = if let Some(usage) = token_usage {
            let CodingAgentTotalTokenUsage(usage) = usage;
            format!(
                " 🎯 Input: {} | Output: {} | Cache Read: {} | Cache Write: {}",
                usage.input, usage.output, usage.cache_read, usage.cache_write
            )
        } else {
            " 🎯 Token usage: Waiting for first response...".to_string()
        };
        let status = Paragraph::new(status_text)
            .style(Style::default().fg(ratatui::style::Color::Rgb(100, 150, 200)))
            .block(Block::bordered().title("Token Usage"));
        () = frame.render_widget(status, status_area);

        let input_text: Vec<ratatui::text::Line<'_>> = input_lines
            .iter()
            .map(|l| ratatui::text::Line::from(l.as_str()))
            .collect();
        let input_border_color = if self.focused == TuiMainFocus::InputArea {
            COLOR_BORDER_FOCUSED
        } else {
            COLOR_BORDER_UNFOCUSED
        };
        let input = Paragraph::new(input_text).style(Style::default()).block(
            Block::bordered()
                .title("Input")
                .border_style(Style::default().fg(input_border_color)),
        );
        () = frame.render_widget::<Paragraph<'_>>(input, input_area);

        if self.show_cursor && self.focused == TuiMainFocus::InputArea {
            let cursor_total = PROMPT_PREFIX.width() + self.display_index();
            // Walk actual line widths instead of dividing by inner_width.
            // Wide chars can leave a gap at line end, making simple division wrong.
            let (cursor_row, cursor_col) = {
                let mut accumulated = 0usize;
                let mut result = (0usize, cursor_total);
                for (row, line) in input_lines.iter().enumerate() {
                    let line_w = line.width();
                    if cursor_total <= accumulated + line_w {
                        result = (row, cursor_total - accumulated);
                        break;
                    }
                    if row + 1 < input_lines.len() {
                        accumulated += line_w;
                    } else {
                        result = (row, cursor_total - accumulated);
                    }
                }
                result
            };
            () = frame.set_cursor_position((
                input_area.left() + cursor_col as u16 + 1,
                input_area.top() + cursor_row as u16 + 1,
            ));
        }
    }

    fn scroll_up(&mut self) {
        self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        if self.vertical_scroll < self.max_scroll() {
            self.vertical_scroll = self.vertical_scroll.saturating_add(1);
        }
    }

    fn scroll_page_up(&mut self) {
        self.vertical_scroll = self
            .vertical_scroll
            .saturating_sub(self.output_area_height());
    }

    fn scroll_page_down(&mut self) {
        self.vertical_scroll =
            (self.vertical_scroll + self.output_area_height()).min(self.max_scroll());
    }

    fn scroll_to_top(&mut self) {
        self.vertical_scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.vertical_scroll = self.max_scroll();
    }
}

fn handle_global_input(
    mut messages: MessageReader<KeyMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut next_tui_main_focus: ResMut<NextState<TuiMainFocus>>,
    mut exit: MessageWriter<AppExit>,
    focus: Res<State<TuiMainFocus>>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    for message in messages.read() {
        let KeyEvent {
            code,
            kind,
            modifiers,
            ..
        } = &**message;

        let in_input = focus.get() == &TuiMainFocus::InputArea;

        match code {
            KeyCode::Tab => {
                let TuiMain { focused, .. } = tui_main.as_mut();
                let next = focused.next().unwrap();
                () = next_tui_main_focus.set(next);
                **dirty = true;
            }
            KeyCode::Char('c')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                _ = exit.write_default();
            }
            // Up/Down/Home/End are yielded to InputArea for cursor/history when focused there.
            KeyCode::Up
                if !in_input && (kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat) =>
            {
                () = tui_main.scroll_up();
                **dirty = true;
            }
            KeyCode::Down
                if !in_input && (kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat) =>
            {
                () = tui_main.scroll_down();
                **dirty = true;
            }
            KeyCode::PageUp => {
                () = tui_main.scroll_page_up();
                **dirty = true;
            }
            KeyCode::PageDown => {
                () = tui_main.scroll_page_down();
                **dirty = true;
            }
            KeyCode::Home if !in_input => {
                () = tui_main.scroll_to_top();
                **dirty = true;
            }
            KeyCode::End if !in_input => {
                () = tui_main.scroll_to_bottom();
                **dirty = true;
            }
            // Toggle ThinkingBlock — targets the selected ResponseBlock when one is
            // selected, otherwise falls through to the last response turn.
            // Works from any focus area so the user needn't Tab after clicking.
            KeyCode::Char('t') if !in_input && matches!(kind, KeyEventKind::Press) => {
                () = tui_main.toggle_last_thinking();
                **dirty = true;
            }
            // Expand / collapse all ThinkingBlocks across every response turn.
            KeyCode::Char('A') if !in_input && matches!(kind, KeyEventKind::Press) => {
                () = tui_main.expand_all_thinking();
                **dirty = true;
            }
            KeyCode::Char('a') if !in_input && matches!(kind, KeyEventKind::Press) => {
                () = tui_main.collapse_all_thinking();
                **dirty = true;
            }
            _ => (),
        }
    }
}

fn parse_commit_message(arg: &str) -> String {
    let arg = arg.trim();
    if let Some(after_m) = arg.strip_prefix("-m") {
        after_m
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string()
    } else if let Some(idx) = arg.find("-m ") {
        arg[idx + 3..]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string()
    } else {
        String::new()
    }
}

fn help_lines() -> Vec<Line<'static>> {
    vec![
        Line::raw(""),
        Line::from("Commands (in REPL):".bold()),
        Line::raw(""),
        Line::from("  Session:".cyan().bold()),
        Line::raw("    /help              Show this help"),
        Line::raw("    /clear             Clear output"),
        Line::raw("    /quit, /exit       Exit greatsage"),
        Line::raw(""),
        Line::from("  Git:".cyan().bold()),
        Line::raw("    /git stage         Stage all changes"),
        Line::raw("    /git commit -m …   Commit staged changes"),
        Line::raw("    /git revert        Revert last commit"),
        Line::raw(""),
        Line::from("  Keyboard shortcuts:".cyan().bold()),
        Line::raw("    Tab                Switch focus (Input ↔ Output)"),
        Line::raw("    ↑/↓  PgUp/PgDn    Scroll output (in Output area)"),
        Line::raw("    t                  Toggle last thinking block (Output area)"),
        Line::raw("    A                  Expand all thinking blocks (Output area)"),
        Line::raw("    a                  Collapse all thinking blocks (Output area)"),
        Line::raw("    Ctrl+C             Exit"),
        Line::raw(""),
    ]
}

fn handle_git_subcmd(cmd: &str) -> Vec<Line<'static>> {
    let rest = cmd.trim_start_matches("/git").trim();
    let (subcmd, arg) = rest
        .split_once(' ')
        .map(|(s, a)| (s.trim(), a.trim()))
        .unwrap_or((rest, ""));

    match subcmd {
        "stage" => match crate::git::stage_all() {
            Ok(_) => vec![Line::from("✅ Staged all changes").green()],
            Err(e) => vec![Line::from(format!("❌ /git stage failed: {e}")).red()],
        },
        "commit" => {
            let msg = parse_commit_message(arg);
            if msg.is_empty() {
                vec![Line::from("❌ /git commit missing -m message").red()]
            } else {
                match crate::git::commit(&msg) {
                    Ok(_) => vec![Line::from(format!("✅ Commit: {msg}")).green()],
                    Err(e) => vec![Line::from(format!("❌ /git commit failed: {e}")).red()],
                }
            }
        }
        "revert" => match crate::git::revert_last() {
            Ok(_) => vec![Line::from("✅ Reverted last commit").green()],
            Err(e) => vec![Line::from(format!("❌ /git revert failed: {e}")).red()],
        },
        "" => help_lines(),
        _ => vec![Line::from(format!("❌ Unknown /git subcommand: {subcmd}")).red()],
    }
}

pub(crate) fn handle_slash_command(cmd: &str) -> Vec<Line<'static>> {
    let base = cmd.split_whitespace().next().unwrap_or(cmd);
    match base {
        "/help" => help_lines(),
        "/git" => handle_git_subcmd(cmd),
        _ => vec![Line::from(format!("❌ Unknown command: {cmd}")).red()],
    }
}

fn handle_input_area_input(
    mut messages: MessageReader<KeyMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
    mut exit: MessageWriter<AppExit>,
    channel: Res<CodingAgentPromptChannel>,
) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    for message in messages.read() {
        let KeyEvent {
            code,
            kind,
            modifiers,
            ..
        } = &**message;

        let is_active = kind == &KeyEventKind::Press || kind == &KeyEventKind::Repeat;

        match code {
            KeyCode::Char(c) if is_active && !modifiers.contains(KeyModifiers::CONTROL) => {
                tui_main.insert_char(*c);
                **dirty = true;
            }
            KeyCode::Char('a')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                tui_main.cursor_to_start();
                **dirty = true;
            }
            KeyCode::Char('e')
                if matches!(kind, KeyEventKind::Press)
                    && modifiers.contains(KeyModifiers::CONTROL) =>
            {
                tui_main.cursor_to_end();
                **dirty = true;
            }
            KeyCode::Backspace if is_active => {
                tui_main.delete_before();
                **dirty = true;
            }
            KeyCode::Delete if is_active => {
                tui_main.delete_after();
                **dirty = true;
            }
            KeyCode::Left if is_active => {
                tui_main.cursor_left();
                **dirty = true;
            }
            KeyCode::Right if is_active => {
                tui_main.cursor_right();
                **dirty = true;
            }
            KeyCode::Home if matches!(kind, KeyEventKind::Press) => {
                tui_main.cursor_to_start();
                **dirty = true;
            }
            KeyCode::End if matches!(kind, KeyEventKind::Press) => {
                tui_main.cursor_to_end();
                **dirty = true;
            }
            KeyCode::Up if matches!(kind, KeyEventKind::Press) => {
                tui_main.history_prev();
                **dirty = true;
            }
            KeyCode::Down if matches!(kind, KeyEventKind::Press) => {
                tui_main.history_next();
                **dirty = true;
            }
            KeyCode::Enter if matches!(kind, KeyEventKind::Press) => {
                if !tui_main.input.is_empty() {
                    let input = tui_main.input.clone();
                    let trimmed = input.trim();

                    match trimmed {
                        "/exit" | "/quit" => {
                            () = tui_main.clear_input();
                            _ = exit.write_default();
                        }
                        "/clear" => {
                            () = tui_main.clear_output();
                            () = tui_main.clear_input();
                        }
                        cmd if cmd.starts_with('/') => {
                            let lines = handle_slash_command(cmd);
                            () = tui_main.push_history(&input);
                            () = tui_main.push_lines(lines);
                            () = tui_main.clear_input();
                            () = tui_main.scroll_to_bottom();
                        }
                        _ => {
                            () = tui_main.push_history(&input);
                            () = tui_main.push_line(Line::from(format!("> {input}")).dark_gray());
                            () = tui_main.clear_input();
                            () = tui_main.scroll_to_bottom();
                            match channel.sender.send(input) {
                                Ok(_) => {}
                                Err(e) => {
                                    eprintln!("Failed to send REPL input to agent: {e}");
                                    let err_line =
                                        Line::from(format!("❌ Failed to send input: {e}")).red();
                                    () = tui_main.push_line(err_line);
                                    () = tui_main.scroll_to_bottom();
                                }
                            }
                        }
                    }
                }
                **dirty = true;
            }
            _ => (),
        }
    }
}

fn handle_output_area_input(
    mut messages: MessageReader<KeyMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
) {
    use crossterm::event::{KeyCode, KeyEvent};

    for message in messages.read() {
        let KeyEvent { code, .. } = &**message;

        match code {
            KeyCode::Char(' ') => {
                () = tui_main.scroll_page_down();
                **dirty = true;
            }
            _ => (),
        }
    }
}

fn handle_mouse_input(
    mut messages: MessageReader<MouseMessage>,
    mut tui_main: NonSendMut<TuiMain>,
    mut dirty: ResMut<RenderNeeded>,
) {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Position;

    for message in messages.read() {
        let MouseEvent {
            kind, column, row, ..
        } = &**message;
        match kind {
            MouseEventKind::ScrollUp => {
                () = tui_main.scroll_up();
                **dirty = true;
            }
            MouseEventKind::ScrollDown => {
                () = tui_main.scroll_down();
                **dirty = true;
            }
            // Left-click inside the output area.
            //   • Click a ThinkingBlock header → toggle that thinking (and
            //     select the owning ResponseBlock if not already selected).
            //   • Click anywhere else in a ResponseBlock → select / deselect.
            MouseEventKind::Down(MouseButton::Left) => {
                let output_area = tui_main.output_area;
                if output_area.contains(Position {
                    x: *column,
                    y: *row,
                }) {
                    // Convert terminal row to a wrapped-line index, accounting
                    // for the 1-row top border and the current scroll offset.
                    let inner_row = (*row).saturating_sub(output_area.top() + 1) as usize;
                    let map_row = inner_row + tui_main.vertical_scroll;

                    if let Some(Some((block_idx, action))) = tui_main.line_map.get(map_row).copied()
                    {
                        match action {
                            ClickAction::ToggleThinking(ti) => {
                                // Toggle the specific ThinkingBlock that was clicked.
                                if let Some(OutputBlock::Response(resp)) =
                                    tui_main.blocks.get_mut(block_idx)
                                {
                                    if let Some(tb) = resp.thinkings.get_mut(ti) {
                                        if !tb.streaming {
                                            tb.expanded = !tb.expanded;
                                        }
                                    }
                                }
                                // Also (re-)select the block so `t` targets it next.
                                tui_main.selected_block = Some(block_idx);
                            }
                            ClickAction::Select => {
                                // Toggle selection: same block → deselect; new → select.
                                tui_main.selected_block =
                                    if tui_main.selected_block == Some(block_idx) {
                                        None
                                    } else {
                                        Some(block_idx)
                                    };
                            }
                        }
                    }
                    **dirty = true;
                }
            }
            _ => (),
        }
    }
}

fn draw_scene_system(
    mut context: ResMut<RatatuiContext>,
    mut tui: NonSendMut<TuiMain>,
    time: Res<Time<()>>,
    mut cursor_timer: Local<Option<Timer>>,
    mut spinner_timer: Local<Option<Timer>>,
    mut dirty: ResMut<RenderNeeded>,
    token_usage: Option<Res<CodingAgentTotalTokenUsage>>,
) -> bevy::ecs::error::Result {
    // Cursor blink timer: toggle show_cursor every 530 ms.
    let cursor_timer = cursor_timer.get_or_insert(Timer::new(
        Duration::from_millis(CURSOR_BLINK_INTERVAL_MS),
        TimerMode::Repeating,
    ));
    _ = cursor_timer.tick(time.delta());
    if cursor_timer.just_finished() {
        tui.show_cursor ^= true;
        **dirty = true;
    }

    // Spinner timer: animate ThinkingBlock / ToolCall spinners at 100 ms intervals.
    // Using a dedicated timer (not every Bevy frame) prevents continuous full-frame
    // redraws that caused visible input-area flicker during agent responses.
    let spinner_timer =
        spinner_timer.get_or_insert(Timer::new(Duration::from_millis(100), TimerMode::Repeating));
    _ = spinner_timer.tick(time.delta());
    let has_spinner = tui.blocks.iter().any(|b| match b {
        OutputBlock::Response(resp) => resp.has_spinner(),
        _ => false,
    });
    if has_spinner && spinner_timer.just_finished() {
        **dirty = true;
    }

    if **dirty {
        _ = context.draw(|frame| {
            () = tui.draw(frame, token_usage.as_deref());
        })?;
    }

    **dirty = false;

    Ok(())
}

/// Restore the block cursor shape that crossterm/raw-mode may have overridden.
/// Blinking is driven by our software timer (show_cursor toggle); the cursor
/// shape itself should be a steady block so the show/hide cycle looks correct.
fn setup_cursor(_: bevy::ecs::system::Commands) {
    let _ = execute!(stdout(), SetCursorStyle::SteadyBlock);
}

pub fn plugin(app: &mut App) {
    _ = app
        .init_non_send_resource::<TuiMain>()
        .init_state::<TuiMainFocus>()
        .add_systems(bevy::app::Startup, setup_cursor)
        .add_systems(
            PreUpdate,
            (
                handle_global_input,
                handle_mouse_input,
                handle_input_area_input.run_if(in_state(TuiMainFocus::InputArea)),
                handle_output_area_input.run_if(in_state(TuiMainFocus::OutputArea)),
            )
                .chain(),
        )
        .add_systems(Update, draw_scene_system);
}
