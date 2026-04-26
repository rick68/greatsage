use {
    bevy::prelude::*,
    ratatui::{
        prelude::Stylize,
        layout::Rect,
        text::Line,
        widgets::ScrollbarState,
    },
    std::time::Instant,
    strum::{EnumCount, FromRepr},
};

pub const PROMPT_PREFIX: &str = "🤖 > ";
pub const SPINNER: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
pub const CURSOR_BLINK_INTERVAL_MS: u64 = 530;

// Border colours
pub const COLOR_BORDER_FOCUSED: ratatui::style::Color = ratatui::style::Color::White;
pub const COLOR_BORDER_UNFOCUSED: ratatui::style::Color = ratatui::style::Color::DarkGray;

/// TUI Focus Area
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
impl std::iter::ExactSizeIterator for TuiMainFocus {}
impl std::iter::DoubleEndedIterator for TuiMainFocus {
    fn next_back(&mut self) -> Option<Self::Item> {
        let next = if (*self as u8) == 0 { (Self::COUNT - 1) as u8 } else { (*self as u8) - 1 };
        TuiMainFocus::from_repr(next).inspect(|item| *self = *item)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ClickAction {
    Select,
    ToggleThinking(usize),
}

pub struct ThinkingBlock {
    pub raw: String,
    pub lines: Vec<Line<'static>>,
    pub elapsed_secs: f32,
    pub token_count: u32,
    pub expanded: bool,
    pub streaming: bool,
    pub start_instant: Instant,
}

impl ThinkingBlock {
    pub fn new() -> Self {
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
}

pub struct ToolCallEntry {
    pub summary: String,
    pub running: bool,
    pub is_error: bool,
    pub error_snippet: String,
    pub start_instant: Instant,
}

pub struct ResponseBlock {
    pub thinkings: Vec<ThinkingBlock>,
    pub tool_calls: Vec<ToolCallEntry>,
    pub text_lines: Vec<Line<'static>>,
    pub text_streaming: bool,
    pub sealed: bool,
}

impl ResponseBlock {
    pub fn new() -> Self {
        Self {
            thinkings: Vec::new(),
            tool_calls: Vec::new(),
            text_lines: Vec::new(),
            text_streaming: false,
            sealed: false,
        }
    }
    pub fn has_spinner(&self) -> bool {
        self.thinkings.iter().any(|tb| tb.streaming) || self.tool_calls.iter().any(|tc| tc.running)
    }
}

pub enum OutputBlock {
    Lines(Vec<Line<'static>>),
    Response(ResponseBlock),
}

pub struct TuiMain {
    pub input: String,
    pub byte_index: usize,
    pub history: Vec<String>,
    pub history_pos: Option<usize>,
    pub history_draft: String,
    pub blocks: Vec<OutputBlock>,
    pub output_area: Rect,
    pub show_cursor: bool,
    pub focused: TuiMainFocus,
    pub vertical_scroll: usize,
    pub vertical_scroll_state: ScrollbarState,
    pub line_map: Vec<Option<(usize, ClickAction)>>,
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

impl TuiMain {
    pub fn max_scroll(&self) -> usize {
        self.total_visual_rows().saturating_sub(self.output_area_height())
    }

    pub fn scroll_to_bottom(&mut self) {
        let max = self.max_scroll();
        self.vertical_scroll = max;
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.byte_index, c);
        self.byte_index += c.len_utf8();
    }

    pub fn delete_before(&mut self) {
        if self.byte_index == 0 { return; }
        let c = self.input[..self.byte_index].chars().next_back().unwrap();
        self.byte_index -= c.len_utf8();
        self.input.remove(self.byte_index);
    }

    pub fn delete_after(&mut self) {
        if self.byte_index < self.input.len() { self.input.remove(self.byte_index); }
    }

    pub fn cursor_left(&mut self) {
        if let Some(c) = self.input[..self.byte_index].chars().next_back() {
            self.byte_index -= c.len_utf8();
        }
    }

    pub fn cursor_right(&mut self) {
        if let Some(c) = self.input[self.byte_index..].chars().next() {
            self.byte_index += c.len_utf8();
        }
    }

    pub fn cursor_to_start(&mut self) { self.byte_index = 0; }
    pub fn cursor_to_end(&mut self) { self.byte_index = self.input.len(); }

    pub fn set_input(&mut self, s: String) {
        self.input = s;
        self.cursor_to_end();
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        self.byte_index = 0;
    }

    pub fn clear_output(&mut self) {
        self.blocks.clear();
        self.vertical_scroll = 0;
        self.vertical_scroll_state = ScrollbarState::default();
    }

    pub fn push_history(&mut self, s: &str) {
        if !s.is_empty() && self.history.last().map(|l| l != s).unwrap_or(true) {
            self.history.push(s.to_owned());
        }
        self.history_pos = None;
        self.history_draft.clear();
    }

    /// Returns the last item in the history.
    #[cfg(test)]
    pub fn last_history(&self) -> Option<&str> {
        self.history.last().map(|s| s.as_str())
    }

    /// Returns true if the input buffer is empty.
    #[cfg(test)]
    pub fn input_is_empty(&self) -> bool {
        self.input.is_empty()
    }

    pub fn history_prev(&mut self) {
        if self.history.is_empty() { return; }
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

    pub fn history_next(&mut self) {
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

    pub fn output_area_height(&self) -> usize {
        self.output_area.height.saturating_sub(2) as usize
    }

    pub fn scroll_up(&mut self) {
        self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        let max = self.max_scroll();
        if self.vertical_scroll < max { self.vertical_scroll += 1; }
    }

    #[allow(dead_code)]
    pub fn scroll_page_up(&mut self) {
        let h = self.output_area_height();
        self.vertical_scroll = self.vertical_scroll.saturating_sub(h);
    }

    #[allow(dead_code)]
    pub fn scroll_page_down(&mut self) {
        let h = self.output_area_height();
        let max = self.max_scroll();
        self.vertical_scroll = (self.vertical_scroll + h).min(max);
    }

    pub fn scroll_to_top(&mut self) { self.vertical_scroll = 0; }

    pub fn total_visual_rows(&self) -> usize {
        use unicode_width::UnicodeWidthChar;
        let inner_width = self.output_area.width.saturating_sub(2) as usize;
        let (flat, _) = crate::tui::renderer::layout_utils::rendered_flat_lines(self, &SPINNER);
        if inner_width == 0 { return flat.len(); }
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

    pub fn toggle_last_thinking(&mut self) {
        let target = self.selected_block.or_else(|| {
            self.blocks.iter().rposition(|b| matches!(b, OutputBlock::Response(_)))
        });
        if let Some(idx) = target.and_then(|idx| {
            if let Some(OutputBlock::Response(resp)) = self.blocks.get_mut(idx) {
                Some((idx, resp))
            } else {
                None
            }
        }) {
            let (_, resp) = idx;
            for tb in resp.thinkings.iter_mut().rev() {
                if !tb.streaming {
                    tb.expanded = !tb.expanded;
                    return;
                }
            }
        }
    }

    pub fn expand_all_thinking(&mut self) {
        for block in self.blocks.iter_mut() {
            if let OutputBlock::Response(resp) = block {
                for tb in resp.thinkings.iter_mut() { if !tb.streaming { tb.expanded = true; } }
            }
        }
    }

    pub fn collapse_all_thinking(&mut self) {
        for block in self.blocks.iter_mut() {
            if let OutputBlock::Response(resp) = block {
                for tb in resp.thinkings.iter_mut() { if !tb.streaming { tb.expanded = false; } }
            }
        }
    }

    pub fn push_line(&mut self, line: Line<'static>) {
        match self.blocks.last_mut() {
            Some(OutputBlock::Lines(lines)) => lines.push(line),
            _ => self.blocks.push(OutputBlock::Lines(vec![line])),
        }
    }

    pub fn push_lines(&mut self, new_lines: Vec<Line<'static>>) {
        match self.blocks.last_mut() {
            Some(OutputBlock::Lines(lines)) => lines.extend(new_lines),
            _ => self.blocks.push(OutputBlock::Lines(new_lines)),
        }
    }

    pub fn current_response_mut(&mut self) -> Option<&mut ResponseBlock> {
        self.blocks.iter_mut().rev().find_map(|b| {
            if let OutputBlock::Response(r) = b { Some(r) } else { None }
        })
    }

    pub fn begin_response(&mut self) {
        self.blocks.push(OutputBlock::Response(ResponseBlock::new()));
    }

    pub fn end_response(&mut self) {
        if let Some(resp) = self.current_response_mut() { resp.sealed = true; }
    }

    pub fn begin_thinking(&mut self) {
        if let Some(resp) = self.current_response_mut() { resp.thinkings.push(ThinkingBlock::new()); }
    }

    pub fn append_thinking(&mut self, raw: &str) {
        if let Some(tb) = self.current_response_mut().and_then(|resp| {
            resp.thinkings.iter_mut().rev().find(|tb| tb.streaming)
        }) {
            tb.raw.push_str(raw);
            tb.lines = tb.raw.lines().map(|l| Line::from(ratatui::text::Span::from(l.to_string()).dim())).collect();
        }
    }

    pub fn end_thinking(&mut self, token_count: u32) {
        if let Some(tb) = self.current_response_mut().and_then(|resp| {
            resp.thinkings.iter_mut().rev().find(|tb| tb.streaming)
        }) {
            tb.elapsed_secs = tb.start_instant.elapsed().as_secs_f32();
            tb.token_count = token_count;
            tb.streaming = false;
        }
    }

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

    pub fn finish_tool_call(&mut self, is_error: bool, error_snippet: String) {
        if let Some(tc) = self.current_response_mut().and_then(|resp| {
            resp.tool_calls.iter_mut().rev().find(|tc| tc.running)
        }) {
            tc.running = false;
            tc.is_error = is_error;
            tc.error_snippet = error_snippet;
        }
    }

    pub fn begin_streaming_text(&mut self) {
        if let Some(resp) = self.current_response_mut() { resp.text_streaming = true; }
    }

    pub fn update_streaming_text(&mut self, new_lines: Vec<Line<'static>>) {
        if let Some(resp) = self.current_response_mut() { resp.text_lines = new_lines; }
    }

    pub fn finalize_streaming_text(&mut self) {
        if let Some(resp) = self.current_response_mut() { resp.text_streaming = false; }
    }
}
