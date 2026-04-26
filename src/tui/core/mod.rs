//! Core TUI state — data structures and mutation API.
//!
//! This module owns all persistent UI state that survives across frames:
//! the input buffer, command history, output blocks, scroll position, and
//! the line→block mapping used by the mouse click handler.
//!
//! # Block model
//!
//! Output is stored as a flat `Vec<OutputBlock>`.  There are two kinds:
//!
//! * [`OutputBlock::Lines`] — arbitrary styled lines (system messages, prompts, …)
//! * [`OutputBlock::Response`] — one complete agent turn, containing any mix of
//!   [`ThinkingBlock`]s, [`ToolCallEntry`]s, and streamed/finalized text lines.
//!
//! The [`renderer`] module flattens this tree into a single `Vec<Line>` every
//! frame, then word-wraps it to the current terminal width.  A parallel
//! `line_map` vector records which block and [`ClickAction`] corresponds to
//! each visual row, enabling mouse-click routing.
//!
//! [`renderer`]: crate::tui::renderer

use {
    bevy::state::state::States,
    ratatui::{layout::Rect, prelude::Stylize, text::Line, widgets::ScrollbarState},
    std::{
        iter::{DoubleEndedIterator, ExactSizeIterator, Iterator},
        time::Instant,
    },
    strum::{EnumCount, FromRepr},
};

// ── Constants ──────────────────────────────────────────────────────────────────

/// String prepended to every line of user input in the input box.
pub const PROMPT_PREFIX: &str = "🤖 > ";

/// Braille-dot spinner frames, cycled at ~100 ms per frame.
pub const SPINNER: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

/// How long (milliseconds) each phase of the software cursor blink lasts.
pub const CURSOR_BLINK_INTERVAL_MS: u64 = 530;

/// Border color for the panel that currently has keyboard focus (bright white).
///
/// Change this constant to restyle focused borders project-wide.
pub const COLOR_BORDER_FOCUSED: ratatui::style::Color = ratatui::style::Color::White;

/// Border color for panels that do **not** have focus (muted dark gray).
///
/// Change this constant to restyle unfocused borders project-wide.
pub const COLOR_BORDER_UNFOCUSED: ratatui::style::Color = ratatui::style::Color::DarkGray;

// ── Focus state ────────────────────────────────────────────────────────────────

/// Which panel currently receives keyboard input.
///
/// Stored as a Bevy [`States`] value so run-conditions can gate systems on it
/// (e.g., `handle_input_area_input` only runs in `InputArea`).
///
/// Press **Tab** to cycle between the two variants.
///
/// [`States`]: bevy::state::state::States
#[derive(Clone, Copy, Debug, Default, EnumCount, Eq, FromRepr, Hash, PartialEq, States)]
#[repr(u8)]
pub enum TuiMainFocus {
    /// The bottom text-entry box.  Default on startup.
    #[default]
    InputArea,
    /// The scrollable output panel above the input box.
    OutputArea,
}

/// Allows [`TuiMainFocus`] to be cycled with `.next()` / `.next_back()`.
///
/// The iterator wraps around (modular arithmetic over `COUNT`), so calling
/// `next()` on the last variant returns the first one.
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

// ── Click action ───────────────────────────────────────────────────────────────

/// What should happen when the user left-clicks a visual row.
///
/// Every entry in [`TuiMain::line_map`] carries one of these values (or
/// `None` for rows that belong to a plain [`OutputBlock::Lines`] block).
/// The mouse handler in [`crate::tui::input`] reads this to decide which
/// [`TuiAction`] to emit.
///
/// [`TuiAction`]: crate::tui::events::TuiAction
#[derive(Clone, Copy, PartialEq)]
pub enum ClickAction {
    /// Select (or deselect) the parent [`ResponseBlock`].
    ///
    /// Emits [`TuiAction::SelectBlock`].
    ///
    /// [`TuiAction::SelectBlock`]: crate::tui::events::TuiAction::SelectBlock
    Select,
    /// Toggle the thinking block at index `usize` inside the parent [`ResponseBlock`].
    ///
    /// Emits [`TuiAction::ToggleThinking`].  Assigned to the header row (▶/▼)
    /// of each [`ThinkingBlock`].
    ///
    /// [`TuiAction::ToggleThinking`]: crate::tui::events::TuiAction::ToggleThinking
    ToggleThinking(usize),
}

// ── ThinkingBlock ──────────────────────────────────────────────────────────────

/// A single "extended thinking" block produced by the LLM during one turn.
///
/// While the LLM is still streaming its thoughts, `streaming = true` and the
/// header shows a live spinner + elapsed time.  Once `end_thinking()` is
/// called, `streaming` becomes `false`, `elapsed_secs` and `token_count` are
/// frozen, and the header shows ▶/▼ to indicate that it can be expanded.
pub struct ThinkingBlock {
    /// Raw UTF-8 text accumulated during streaming; used to rebuild `lines`.
    pub raw: String,
    /// Pre-rendered styled lines derived from `raw` (dim styling).
    pub lines: Vec<Line<'static>>,
    /// Wall-clock seconds from stream-start to stream-end.
    pub elapsed_secs: f32,
    /// Number of thinking tokens reported by the API at stream-end.
    pub token_count: u32,
    /// Whether the body is currently shown (`true`) or hidden (`false`).
    pub expanded: bool,
    /// `true` while the LLM is still producing thinking tokens.
    pub streaming: bool,
    /// Monotonic timestamp taken at construction; used to compute elapsed time.
    pub start_instant: Instant,
}

impl ThinkingBlock {
    /// Creates a new, empty, streaming thinking block.
    pub fn new() -> Self {
        Self {
            raw: String::new(),
            lines: Vec::new(),
            elapsed_secs: 0.0,
            token_count: 0,
            expanded: false, // collapsed by default
            streaming: true,
            start_instant: Instant::now(),
        }
    }
}

// ── ToolCallEntry ──────────────────────────────────────────────────────────────

/// A single tool-call invocation within a [`ResponseBlock`].
///
/// While `running` is `true`, the renderer shows an animated spinner.
/// After the tool returns, `running` becomes `false` and either `is_error`
/// or a success icon is shown alongside the elapsed time.
pub struct ToolCallEntry {
    /// Human-readable one-liner shown in the output, e.g. `"🔧 bash(ls -la)"`.
    pub summary: String,
    /// `true` while the tool is still executing.
    pub running: bool,
    /// `true` if the tool returned a non-zero exit code or an error result.
    pub is_error: bool,
    /// Short snippet from stderr / error output (shown only when `is_error`).
    pub error_snippet: String,
    /// Monotonic timestamp taken when the tool call was enqueued.
    pub start_instant: Instant,
}

// ── ResponseBlock ──────────────────────────────────────────────────────────────

/// All content produced by one agent turn, grouped into a single visual unit.
///
/// A response block is created by [`TuiMain::begin_response`] and closed by
/// [`TuiMain::end_response`] (`sealed = true`).  While open, the streaming
/// helpers append to its fields.
///
/// The renderer draws the block with:
/// * Zero or more [`ThinkingBlock`] headers (clickable ▶/▼)
/// * Zero or more [`ToolCallEntry`] lines (with spinner / status icons)
/// * The agent's text reply (streamed raw, then replaced with markdown at
///   turn-end via `finalize_streaming_text`)
/// * A horizontal rule at the bottom (cyan + gutter if selected, gray if not)
pub struct ResponseBlock {
    /// Extended-thinking sub-blocks for this turn (in arrival order).
    pub thinkings: Vec<ThinkingBlock>,
    /// Tool invocations made during this turn (in arrival order).
    pub tool_calls: Vec<ToolCallEntry>,
    /// Pre-rendered text lines (markdown or raw streaming text).
    pub text_lines: Vec<Line<'static>>,
    /// `true` while the LLM is still streaming its text reply.
    pub text_streaming: bool,
    /// `true` after [`TuiMain::end_response`] is called; prevents further mutation.
    pub sealed: bool,
}

impl ResponseBlock {
    /// Creates a new, empty, unsealed response block.
    pub fn new() -> Self {
        Self {
            thinkings: Vec::new(),
            tool_calls: Vec::new(),
            text_lines: Vec::new(),
            text_streaming: false,
            sealed: false,
        }
    }

    /// Returns `true` if any sub-element is still animating (thinking stream
    /// or running tool call), which drives the spinner refresh timer.
    pub fn has_spinner(&self) -> bool {
        self.thinkings.iter().any(|tb| tb.streaming) || self.tool_calls.iter().any(|tc| tc.running)
    }
}

// ── OutputBlock ────────────────────────────────────────────────────────────────

/// A single entry in the [`TuiMain::blocks`] list.
///
/// The output panel is a flat list of these; the renderer iterates them to
/// produce the final [`Vec<Line>`] that Ratatui displays.
pub enum OutputBlock {
    /// Plain styled lines — system messages, prompts, slash-command output, …
    Lines(Vec<Line<'static>>),
    /// One complete (or in-progress) agent turn, rendered as a [`ResponseBlock`].
    Response(ResponseBlock),
}

// ── TuiMain ────────────────────────────────────────────────────────────────────

/// Root state of the terminal UI.
///
/// Stored as a Bevy **non-send resource** because it contains types (`Instant`,
/// raw `Vec` internals) that are not `Send`.  Access it via
/// `NonSendMut<TuiMain>` / `NonSend<TuiMain>` in Bevy systems.
///
/// All mutation goes through the public methods below; input systems emit
/// [`TuiAction`] messages that the action system dispatches to those methods.
///
/// [`TuiAction`]: crate::tui::events::TuiAction
pub struct TuiMain {
    // ── Input buffer ──────────────────────────────────────────────────────
    /// Current text in the input box (UTF-8).
    pub input: String,
    /// Byte offset of the cursor inside `input`.  Always on a char boundary.
    pub byte_index: usize,

    // ── Command history ───────────────────────────────────────────────────
    /// Ordered list of previously submitted commands (most-recent last).
    pub history: Vec<String>,
    /// Index into `history` while navigating up with ↑; `None` when at the live draft.
    pub history_pos: Option<usize>,
    /// Saved draft text so it can be restored when the user navigates back down.
    pub history_draft: String,

    // ── Output blocks ─────────────────────────────────────────────────────
    /// All output content, in display order.
    pub blocks: Vec<OutputBlock>,

    // ── Scroll state ──────────────────────────────────────────────────────
    /// The last [`Rect`] occupied by the output panel; used for scroll math and
    /// mouse-click hit-testing.
    pub output_area: Rect,
    /// Scrollbar widget state (content length, viewport size, position).
    pub vertical_scroll_state: ScrollbarState,
    /// Current scroll offset in wrapped visual rows.
    pub vertical_scroll: usize,

    // ── Cursor blink ──────────────────────────────────────────────────────
    /// Toggled every [`CURSOR_BLINK_INTERVAL_MS`] by the renderer timer.
    pub show_cursor: bool,

    // ── Focus ─────────────────────────────────────────────────────────────
    /// The panel that currently owns keyboard input.
    pub focused: TuiMainFocus,

    // ── Line → block mapping (mouse routing) ──────────────────────────────
    /// Parallel to the wrapped visual rows rendered last frame.
    ///
    /// `line_map[i]` is:
    /// * `None`  — the row belongs to an `OutputBlock::Lines` (no click action)
    /// * `Some((block_idx, action))` — the row belongs to `blocks[block_idx]`;
    ///   `action` says what a left-click should do (select block or toggle thinking).
    ///
    /// Rebuilt every frame by [`renderer::draw_scene_system`] before the frame
    /// is rendered.  Must not be read before the first frame.
    ///
    /// [`renderer::draw_scene_system`]: crate::tui::renderer::draw_scene_system
    pub line_map: Vec<Option<(usize, ClickAction)>>,

    /// Index into `blocks` of the currently highlighted [`ResponseBlock`], or
    /// `None` when nothing is selected.
    ///
    /// Used by:
    /// * `toggle_last_thinking` — operates on this block instead of the last one
    /// * The renderer — draws a cyan gutter on the selected block
    pub selected_block: Option<usize>,
}

impl Default for TuiMain {
    fn default() -> Self {
        Self {
            input: String::new(),
            byte_index: 0,
            history: Vec::new(),
            history_pos: None,
            history_draft: String::new(),
            blocks: Vec::new(),
            output_area: Rect::default(),
            show_cursor: true,
            focused: TuiMainFocus::default(),
            vertical_scroll: 0,
            vertical_scroll_state: ScrollbarState::default(),
            line_map: Vec::new(),
            selected_block: None,
        }
    }
}

pub mod action_system;

// ── TuiMain methods ────────────────────────────────────────────────────────────

impl TuiMain {
    // ── Scroll helpers ────────────────────────────────────────────────────

    /// Returns the maximum valid scroll offset (total rows minus visible rows).
    pub fn max_scroll(&self) -> usize {
        self.total_visual_rows()
            .saturating_sub(self.output_area_height())
    }

    /// Scroll the output to the very last line.
    pub fn scroll_to_bottom(&mut self) {
        let max = self.max_scroll();
        self.vertical_scroll = max;
    }

    // ── Input buffer ──────────────────────────────────────────────────────

    /// Insert `c` at the current cursor position and advance the cursor.
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.byte_index, c);
        self.byte_index += c.len_utf8();
    }

    /// Delete the character immediately before the cursor (Backspace).
    pub fn delete_before(&mut self) {
        if self.byte_index == 0 {
            return;
        }
        let c = self.input[..self.byte_index].chars().next_back().unwrap();
        self.byte_index -= c.len_utf8();
        _ = self.input.remove(self.byte_index);
    }

    /// Delete the character immediately after the cursor (Delete key).
    pub fn delete_after(&mut self) {
        if self.byte_index < self.input.len() {
            _ = self.input.remove(self.byte_index);
        }
    }

    /// Move the cursor one Unicode scalar value to the left.
    pub fn cursor_left(&mut self) {
        if let Some(c) = self.input[..self.byte_index].chars().next_back() {
            self.byte_index -= c.len_utf8();
        }
    }

    /// Move the cursor one Unicode scalar value to the right.
    pub fn cursor_right(&mut self) {
        if let Some(c) = self.input[self.byte_index..].chars().next() {
            self.byte_index += c.len_utf8();
        }
    }

    /// Jump the cursor to the start of the input buffer.
    pub fn cursor_to_start(&mut self) {
        self.byte_index = 0;
    }

    /// Jump the cursor to the end of the input buffer.
    pub fn cursor_to_end(&mut self) {
        self.byte_index = self.input.len();
    }

    /// Replace the input buffer with `s` and move the cursor to the end.
    pub fn set_input(&mut self, s: String) {
        self.input = s;
        () = self.cursor_to_end();
    }

    /// Clear the input buffer and reset the cursor.
    pub fn clear_input(&mut self) {
        self.input.clear();
        self.byte_index = 0;
    }

    /// Clear all output blocks and reset the scroll position.
    pub fn clear_output(&mut self) {
        self.blocks.clear();
        self.vertical_scroll = 0;
        self.vertical_scroll_state = ScrollbarState::default();
    }

    // ── Command history ───────────────────────────────────────────────────

    /// Push `s` to the history list unless it duplicates the last entry.
    ///
    /// Always resets `history_pos` to `None` (back to the live draft).
    pub fn push_history(&mut self, s: impl AsRef<str>) {
        if !s.as_ref().is_empty() && self.history.last().map(|l| l != s.as_ref()).unwrap_or(true) {
            () = self.history.push(s.as_ref().to_owned());
        }
        self.history_pos = None;
        () = self.history_draft.clear();
    }

    /// Returns the last item in the history (test helper).
    #[cfg(test)]
    pub fn last_history(&self) -> Option<&str> {
        self.history.last().map(|s| s.as_str())
    }

    /// Returns `true` if the input buffer is empty (test helper).
    #[cfg(test)]
    pub fn input_is_empty(&self) -> bool {
        self.input.is_empty()
    }

    /// Navigate to the previous history entry (↑ key).
    ///
    /// Saves the current draft on first press so it can be restored later.
    pub fn history_prev(&mut self) {
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
        () = self.set_input(entry);
    }

    /// Navigate to the next history entry (↓ key).
    ///
    /// When reaching the end of history, restores the saved draft.
    pub fn history_next(&mut self) {
        match self.history_pos {
            None => (),
            Some(i) if i + 1 >= self.history.len() => {
                self.history_pos = None;
                let draft = self.history_draft.clone();
                () = self.set_input(draft);
            }
            Some(i) => {
                self.history_pos = Some(i + 1);
                let entry = self.history[i + 1].clone();
                () = self.set_input(entry);
            }
        }
    }

    // ── Scroll ────────────────────────────────────────────────────────────

    /// Height of the output panel in rows, excluding the border (2 rows).
    pub fn output_area_height(&self) -> usize {
        self.output_area.height.saturating_sub(2) as usize
    }

    /// Scroll up by one wrapped visual row.
    pub fn scroll_up(&mut self) {
        self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
    }

    /// Scroll down by one wrapped visual row, clamped to `max_scroll`.
    pub fn scroll_down(&mut self) {
        let max = self.max_scroll();
        if self.vertical_scroll < max {
            self.vertical_scroll += 1;
        }
    }

    /// Scroll up by one full page (output panel height).
    #[allow(dead_code)]
    pub fn scroll_page_up(&mut self) {
        let h = self.output_area_height();
        self.vertical_scroll = self.vertical_scroll.saturating_sub(h);
    }

    /// Scroll down by one full page (output panel height).
    #[allow(dead_code)]
    pub fn scroll_page_down(&mut self) {
        let h = self.output_area_height();
        let max = self.max_scroll();
        self.vertical_scroll = (self.vertical_scroll + h).min(max);
    }

    /// Scroll to the very first line.
    pub fn scroll_to_top(&mut self) {
        self.vertical_scroll = 0;
    }

    /// Count the total number of wrapped visual rows across all output blocks.
    ///
    /// This re-runs the word-wrap calculation (without allocating full `Line`s)
    /// to keep the scroll math in sync with the renderer.  It intentionally
    /// mirrors [`renderer::display_utils::hard_wrap_output_lines_with_map`] so
    /// they always agree.
    pub fn total_visual_rows(&self) -> usize {
        use unicode_width::UnicodeWidthChar;
        let inner_width = self.output_area.width.saturating_sub(2) as usize;
        let (flat, _) = crate::tui::renderer::display_utils::rendered_flat_lines(self, &SPINNER);
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

    // ── Thinking controls ─────────────────────────────────────────────────

    /// Toggle the most recently completed thinking block.
    ///
    /// Priority:
    /// 1. If `selected_block` is set and points to a `Response` block, toggle
    ///    the last non-streaming thinking in that block.
    /// 2. Otherwise, find the last `Response` block and do the same.
    ///
    /// A streaming (in-progress) thinking block is never toggled.
    pub fn toggle_last_thinking(&mut self) {
        // Prefer the user-selected block, fall back to the last ResponseBlock.
        let target = self.selected_block.or_else(|| {
            self.blocks
                .iter()
                .rposition(|b| matches!(b, OutputBlock::Response(_)))
        });
        if let Some(idx) = target.and_then(|idx| {
            if let Some(OutputBlock::Response(resp)) = self.blocks.get_mut(idx) {
                Some((idx, resp))
            } else {
                None
            }
        }) {
            let (_, resp) = idx;
            // Walk backwards so we toggle the most-recent finished thinking.
            for tb in resp.thinkings.iter_mut().rev() {
                if !tb.streaming {
                    tb.expanded = !tb.expanded;
                    return;
                }
            }
        }
    }

    /// Expand every finished thinking block across all response blocks.
    pub fn expand_all_thinking(&mut self) {
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

    /// Collapse every finished thinking block across all response blocks.
    pub fn collapse_all_thinking(&mut self) {
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

    // ── Output building ───────────────────────────────────────────────────

    /// Append a single styled line to the current `Lines` block (or start one).
    pub fn push_line(&mut self, line: Line<'static>) {
        match self.blocks.last_mut() {
            Some(OutputBlock::Lines(lines)) => lines.push(line),
            _ => self.blocks.push(OutputBlock::Lines(vec![line])),
        }
    }

    /// Append multiple styled lines to the current `Lines` block (or start one).
    pub fn push_lines(&mut self, new_lines: Vec<Line<'static>>) {
        match self.blocks.last_mut() {
            Some(OutputBlock::Lines(lines)) => lines.extend(new_lines),
            _ => self.blocks.push(OutputBlock::Lines(new_lines)),
        }
    }

    // ── Response block lifecycle ───────────────────────────────────────────

    /// Returns a mutable reference to the last `ResponseBlock`, if any.
    ///
    /// Used internally by the streaming helpers to target the in-progress turn.
    pub fn current_response_mut(&mut self) -> Option<&mut ResponseBlock> {
        self.blocks.iter_mut().rev().find_map(|b| {
            if let OutputBlock::Response(r) = b {
                Some(r)
            } else {
                None
            }
        })
    }

    /// Start a new agent turn by pushing an empty [`ResponseBlock`].
    ///
    /// Should be called once per turn before any of the streaming helpers.
    pub fn begin_response(&mut self) {
        () = self
            .blocks
            .push(OutputBlock::Response(ResponseBlock::new()));
    }

    /// Seal the current [`ResponseBlock`], marking the turn as complete.
    pub fn end_response(&mut self) {
        if let Some(resp) = self.current_response_mut() {
            resp.sealed = true;
        }
    }

    // ── Thinking streaming ────────────────────────────────────────────────

    /// Begin a new thinking sub-block in the current response.
    pub fn begin_thinking(&mut self) {
        if let Some(resp) = self.current_response_mut() {
            () = resp.thinkings.push(ThinkingBlock::new());
        }
    }

    /// Append raw text to the currently streaming thinking block.
    ///
    /// Rebuilds `lines` from `raw` on every call so the renderer always shows
    /// the latest content.
    pub fn append_thinking(&mut self, raw: impl AsRef<str>) {
        if let Some(tb) = self
            .current_response_mut()
            .and_then(|resp| resp.thinkings.iter_mut().rev().find(|tb| tb.streaming))
        {
            tb.raw.push_str(raw.as_ref());
            // Re-render all lines from the full raw text to avoid partial-line artifacts.
            tb.lines = tb
                .raw
                .lines()
                .map(|l| Line::from(ratatui::text::Span::from(l.to_string()).dim()))
                .collect();
        }
    }

    /// Finalize the currently streaming thinking block.
    ///
    /// Freezes `elapsed_secs` and `token_count`, sets `streaming = false`.
    pub fn end_thinking(&mut self, token_count: u32) {
        if let Some(tb) = self
            .current_response_mut()
            .and_then(|resp| resp.thinkings.iter_mut().rev().find(|tb| tb.streaming))
        {
            tb.elapsed_secs = tb.start_instant.elapsed().as_secs_f32();
            tb.token_count = token_count;
            tb.streaming = false;
        }
    }

    // ── Tool call tracking ────────────────────────────────────────────────

    /// Record the start of a tool execution in the current response.
    ///
    /// `summary` is the one-liner shown in the output (e.g. `"🔧 bash(ls)"`)
    pub fn begin_tool_call(&mut self, summary: String) {
        if let Some(resp) = self.current_response_mut() {
            () = resp.tool_calls.push(ToolCallEntry {
                summary,
                running: true,
                is_error: false,
                error_snippet: String::new(),
                start_instant: Instant::now(),
            });
        }
    }

    /// Mark the most recent running tool call as finished.
    ///
    /// `is_error` controls whether the ❌ or ✅ icon is shown.
    /// `error_snippet` is displayed inline when `is_error` is `true`.
    pub fn finish_tool_call(&mut self, is_error: bool, error_snippet: String) {
        if let Some(tc) = self
            .current_response_mut()
            .and_then(|resp| resp.tool_calls.iter_mut().rev().find(|tc| tc.running))
        {
            tc.running = false;
            tc.is_error = is_error;
            tc.error_snippet = error_snippet;
        }
    }

    // ── Text streaming ────────────────────────────────────────────────────

    /// Mark the current response as actively streaming text.
    pub fn begin_streaming_text(&mut self) {
        if let Some(resp) = self.current_response_mut() {
            resp.text_streaming = true;
        }
    }

    /// Replace the text lines in the current response.
    ///
    /// Called repeatedly during streaming (with raw lines) and once at the end
    /// with markdown-rendered lines.
    pub fn update_streaming_text(&mut self, new_lines: Vec<Line<'static>>) {
        if let Some(resp) = self.current_response_mut() {
            resp.text_lines = new_lines;
        }
    }

    /// Mark text streaming as complete (text is now fully rendered).
    pub fn finalize_streaming_text(&mut self) {
        if let Some(resp) = self.current_response_mut() {
            resp.text_streaming = false;
        }
    }
}
