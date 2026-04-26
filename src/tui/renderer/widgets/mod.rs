//! Widget-level renderers for the output block sub-elements.
//!
//! Each sub-module renders one logical piece of a [`ResponseBlock`] into a
//! `Vec<Line<'static>>` plus a parallel `Vec<ClickAction>`.  The parent
//! [`response`] module combines them into a complete response rendering.
//!
//! | Module | Renders |
//! |--------|---------|
//! | [`response`] | Whole [`ResponseBlock`]: thinking + tools + text + gutter |
//! | [`thinking`] | Single [`ThinkingBlock`] header (▶/▼ or live spinner) |
//! | [`tool_call`] | Single [`ToolCallEntry`] status line |
//!
//! [`ResponseBlock`]: crate::tui::core::ResponseBlock
//! [`ThinkingBlock`]: crate::tui::core::ThinkingBlock
//! [`ToolCallEntry`]: crate::tui::core::ToolCallEntry

pub mod response;
pub mod thinking;
pub mod tool_call;
