//! Mutable TUI interaction state.

use bevy::ecs::resource::Resource;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TuiFocus {
    #[default]
    Prompt,
    Scrollback,
}

#[derive(Clone, Debug, Resource)]
pub struct TuiState {
    pub focus: TuiFocus,
    pub prompt: String,
    pub cursor: usize,
    /// Scroll offset from bottom (0 = follow tail).
    pub scroll_from_bottom: u16,
    /// Ephemeral status / slash output lines (foundation message area).
    pub status_lines: Vec<String>,
}

impl Default for TuiState {
    fn default() -> Self {
        Self {
            focus: TuiFocus::Prompt,
            prompt: String::new(),
            cursor: 0,
            scroll_from_bottom: 0,
            status_lines: Vec::new(),
        }
    }
}

impl TuiState {
    pub fn push_status(&mut self, line: impl Into<String>) {
        self.status_lines.push(line.into());
        const MAX: usize = 32;
        if self.status_lines.len() > MAX {
            let drop = self.status_lines.len() - MAX;
            self.status_lines.drain(0..drop);
        }
    }

    pub fn clear_prompt(&mut self) {
        () = self.prompt.clear();
        self.cursor = 0;
    }
}
