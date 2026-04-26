use {
    bevy::prelude::Resource,
    bevy::ecs::message::Message,
};

/// Core resource for triggering UI redraws.
#[derive(bevy::prelude::Deref, bevy::prelude::DerefMut, Resource)]
pub struct RenderNeeded(pub bool);

impl Default for RenderNeeded {
    fn default() -> Self {
        Self(true)
    }
}

/// Parsed TUI command events.
#[derive(Message)]
#[allow(dead_code)]
pub struct TuiCommandEvent {
    pub raw: String,
    pub command: String,
    pub args: Vec<String>,
}

/// Internal TUI action events used to decouple input from execution logic.
#[derive(Message)]
pub enum TuiAction {
    InsertChar(char),
    CursorLeft,
    CursorRight,
    CursorToStart,
    CursorToEnd,
    Backspace,
    Delete,
    HistoryPrev,
    HistoryNext,
    Submit,
    ScrollUp,
    ScrollDown,
    ToggleThinking { block_index: usize, thinking_index: usize },
    ToggleLastThinking,
    ExpandAllThinking,
    CollapseAllThinking,
    SelectBlock(usize),
    #[allow(dead_code)]
    DeselectBlock,
}
