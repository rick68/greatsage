//! Event and resource types used to communicate between TUI sub-systems.
//!
//! The central design goal is to **decouple input from execution**: input
//! handlers translate raw crossterm events into [`TuiAction`] messages, and
//! the action system processes those messages the next Bevy tick.  This keeps
//! each system small and independently testable.

use bevy::{
    ecs::{message::Message, resource::Resource},
    prelude::{Deref, DerefMut},
};

/// Dirty flag that controls whether a new frame is rendered.
///
/// Initialized to `true` so the very first frame is always drawn.  Set back to
/// `false` after each render.  Any system that mutates visible state should set
/// this to `true` to trigger a redraw.
#[derive(Deref, DerefMut, Resource)]
pub struct RenderNeeded(pub bool);

impl Default for RenderNeeded {
    fn default() -> Self {
        Self(true)
    }
}

/// A fully-parsed slash-command event.
///
/// Currently produced by [`crate::tui::commands::handle_slash_command`] and
/// stored here for potential future use (e.g., logging, macros, scripting).
#[derive(Message)]
#[allow(dead_code)]
pub struct TuiCommandEvent {
    /// The raw command string as typed by the user, e.g. `"/git stage"`.
    pub raw: String,
    /// The base command token, e.g. `"/git"`.
    pub command: String,
    /// Remaining space-separated tokens, e.g. `["stage"]`.
    pub args: Vec<String>,
}

/// All TUI state-mutation operations, expressed as data.
///
/// Input handlers (keyboard / mouse) **write** these messages; the
/// [`crate::tui::core::action_system::tui_action_system`] **reads** and
/// executes them against [`crate::tui::core::TuiMain`].
///
/// Using a message bus instead of direct calls means input and rendering
/// systems never need mutable access at the same time.
#[derive(Message)]
pub enum TuiAction {
    // ── Text editing ──────────────────────────────────────────────────────
    /// Insert a single Unicode character at the current cursor position.
    InsertChar(char),
    /// Move the cursor one Unicode scalar to the left.
    CursorLeft,
    /// Move the cursor one Unicode scalar to the right.
    CursorRight,
    /// Jump to the beginning of the input buffer (or scroll to top in OutputArea).
    CursorToStart,
    /// Jump to the end of the input buffer (or scroll to bottom in OutputArea).
    CursorToEnd,
    /// Delete the character before the cursor (Backspace).
    Backspace,
    /// Delete the character after the cursor (Delete / Forward-delete).
    Delete,

    // ── Command history ───────────────────────────────────────────────────
    /// Recall the previous history entry (↑).
    HistoryPrev,
    /// Recall the next history entry (↓), restoring the draft if at the end.
    HistoryNext,

    // ── Submit ────────────────────────────────────────────────────────────
    /// Finalize and send the current input to the agent (Enter).
    Submit,

    // ── Output scrolling ──────────────────────────────────────────────────
    /// Scroll the output view up by one line.
    ScrollUp,
    /// Scroll the output view down by one line.
    ScrollDown,
    Quit,

    // ── Thinking-block controls ───────────────────────────────────────────
    /// Toggle a specific thinking block identified by block and thinking index.
    ///
    /// Emitted when the user clicks the ▶/▼ header of a thinking block.
    /// Also selects the parent response block.
    ToggleThinking {
        /// Index into [`TuiMain::blocks`].
        block_index: usize,
        /// Index into [`ResponseBlock::thinkings`].
        thinking_index: usize,
    },
    /// Toggle the most-recently-completed thinking block.
    ///
    /// If a block is selected (`selected_block` is `Some`), operates on that
    /// block's thinking; otherwise falls back to the last ResponseBlock.
    /// Triggered by the `t` key (OutputArea focus required).
    ToggleLastThinking,
    /// Expand every thinking block in every ResponseBlock (`A` key).
    ExpandAllThinking,
    /// Collapse every thinking block in every ResponseBlock (`a` key).
    CollapseAllThinking,

    // ── Block selection ───────────────────────────────────────────────────
    /// Select (or deselect if already selected) the ResponseBlock at `usize`.
    ///
    /// Emitted on left-click inside the output area when the clicked row
    /// belongs to a ResponseBlock.
    SelectBlock(usize),
    SetSelection {
        start: (usize, usize),
        end: (usize, usize),
        click_count: u8,
    },
    ClearSelection,
    CopySelection,
    /// Clear the current block selection.
    #[allow(dead_code)]
    DeselectBlock,
}
