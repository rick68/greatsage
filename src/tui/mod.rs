//! TUI module — terminal user interface for greatsage.
//!
//! # Architecture overview
//!
//! The TUI is built on top of [Ratatui] and [Bevy], wired together through
//! [bevy-ratatui].  The overall data-flow every frame is:
//!
//! ```text
//! crossterm events
//!      │
//!      ▼
//! input::handle_global_input          ← focus-agnostic keys (Tab, Ctrl-C, scroll, t/a/A)
//! input::handle_input_area_input      ← text-editing keys (only when InputArea is focused)
//! input::handle_mouse_input           ← scroll wheel + left-click (line_map lookup)
//!      │  (all emit TuiAction messages)
//!      ▼
//! core::action_system::tui_action_system   ← mutates TuiMain state
//!      │
//!      ▼
//! renderer::draw_scene_system         ← flattens blocks → wraps → renders frame
//! ```
//!
//! # Key sub-modules
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | [`core`] | State structs: [`TuiMain`], [`ResponseBlock`], [`OutputBlock`], … |
//! | [`events`] | Message types: [`TuiAction`], [`TuiCommandEvent`], [`RenderNeeded`] |
//! | [`input`] | Keyboard/mouse → [`TuiAction`] translation |
//! | [`renderer`] | Ratatui frame rendering + cursor blink + spinner |
//! | [`commands`] | Slash-command handlers (`/help`, `/git`, …) |
//!
//! # Focus model
//!
//! [`TuiMainFocus`] is a Bevy [`States`] value that toggles between
//! `InputArea` and `OutputArea` with **Tab**.
//! Focused borders are rendered in [`core::COLOR_BORDER_FOCUSED`] (white);
//! unfocused borders use [`core::COLOR_BORDER_UNFOCUSED`] (dark gray).
//!
//! # Thinking-block keyboard controls
//!
//! These keys are active whenever the **Output area** is focused (not InputArea):
//!
//! | Key | Effect |
//! |-----|--------|
//! | `t` | Toggle the most-recently-completed thinking block.  If a ResponseBlock is selected (highlighted), operates on that block's thinking instead. |
//! | `A` | Expand **all** thinking blocks across every ResponseBlock |
//! | `a` | Collapse **all** thinking blocks across every ResponseBlock |
//!
//! Clicking a thinking-block header (▶/▼) directly toggles that specific block
//! and selects its parent ResponseBlock.
//!
//! [`States`]: bevy::state::state::States

pub mod commands;

pub mod core;
pub use self::core::{CursorState, TuiMain, TuiMainFocus};

pub mod events;
pub use self::events::RenderNeeded;

pub mod input;
pub mod renderer;

#[cfg(test)]
mod tests;

use {
    crate::tui::events::{TuiAction, TuiCommandEvent},
    bevy::{
        app::{App, AppExit, PreUpdate, Startup, Update},
        ecs::{
            change_detection::ResMut,
            message::MessageReader,
            schedule::{IntoScheduleConfigs, common_conditions::resource_exists},
        },
        state::{app::AppExtStates, condition::in_state},
        utils::default,
    },
    bevy_ratatui::{
        RatatuiPlugins,
        crossterm::{cursor::SetCursorStyle, execute},
        event::ResizeMessage,
    },
    std::io::stdout,
};

/// Marks the UI as dirty whenever the terminal is resized so the next frame
/// re-lays-out and re-renders correctly.
fn handle_resize(mut messages: MessageReader<ResizeMessage>, mut dirty: ResMut<RenderNeeded>) {
    if let Some(ResizeMessage(_size)) = messages.read().next() {
        **dirty = true;
    }
}

/// Sets the terminal cursor to a steady block on startup.
///
/// Some terminal emulators inherit a blinking/beam cursor from previous
/// processes; this ensures a consistent appearance.
fn setup_cursor() {
    let _ = execute!(stdout(), SetCursorStyle::SteadyBlock);
}

/// Bevy plugin that wires up the entire TUI.
///
/// Registers resources, message types, input systems (in [`PreUpdate`] so they
/// run before game logic), and the rendering system (in [`Update`]).
///
/// **Mouse capture** is enabled so scroll-wheel and click events reach the
/// input handlers.
pub fn tui_plugin(app: &mut App) {
    _ = app
        // ── Resources & state ──────────────────────────────────────────────
        // RenderNeeded starts as `true` so the first frame is always drawn.
        .init_resource::<RenderNeeded>()
        // TuiMain is !Send (holds Ratatui state), so stored as a non-send resource.
        .init_non_send_resource::<TuiMain>()
        // Focus toggles between InputArea ↔ OutputArea via Tab.
        .init_state::<TuiMainFocus>()
        // Cursor switches from Blink (active) to Breathing (idle).
        .init_state::<CursorState>()
        // Message channels used to decouple input from action execution.
        .add_message::<TuiAction>()
        .add_message::<TuiCommandEvent>()
        .add_message::<AppExit>()
        // ── External plugins ───────────────────────────────────────────────
        .add_plugins(RatatuiPlugins {
            enable_mouse_capture: true,
            enable_input_forwarding: true,
            ..default::<RatatuiPlugins>()
        })
        // ── Startup ────────────────────────────────────────────────────────
        .add_systems(Startup, setup_cursor)
        // ── Input (PreUpdate — runs before tui_action_system) ──────────────
        // Systems are chained so resize is processed before key/mouse events.
        .add_systems(
            PreUpdate,
            (
                handle_resize,
                input::handle_global_input,
                input::handle_mouse_input,
                // Text-editing keys only apply when the input box is focused.
                input::handle_input_area_input.run_if(in_state(TuiMainFocus::InputArea)),
            )
                .chain(),
        )
        // ── Update — action dispatch + rendering ───────────────────────────
        .add_systems(
            Update,
            (
                // Consume TuiAction messages and mutate TuiMain.
                core::action_system::tui_action_system,
                // Render only when RenderNeeded is true (dirty flag).
                renderer::draw_scene_system.run_if(resource_exists::<RenderNeeded>),
            ),
        );
}
