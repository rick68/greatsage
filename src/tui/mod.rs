//! Full-screen TUI (Grok-aligned layout; Session ECS scrollback).
//!
//! Entry: `greatsage tui`. Default `greatsage` remains the line-oriented REPL.
//!
//! **Requires `bevy_ratatui`:** `RatatuiPlugins`, `RatatuiContext`, `event::KeyMessage`.
//! crates.io `0.11.1` targets Bevy 0.18; greatsage uses Bevy 0.19 via git pin of
//! [PR #98](https://github.com/ratatui/bevy_ratatui/pull/98) until a crates.io release.

mod commands;
mod draw;
mod input;
mod layout;
mod nav;
mod slash_complete;

mod plugin;
pub use plugin::tui_plugin;

mod scrollback;
mod state;
