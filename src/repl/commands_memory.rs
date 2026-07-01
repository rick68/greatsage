//! Project memory slash commands: `/remember`, `/memories`, `/forget`.

use {
    super::dispatch::{DispatchResult, ReplDispatchCtx},
    crate::project_memory::{forget_output_lines, memories_output_lines, remember_output_lines},
    std::{env, path::Path},
};

pub(super) fn dispatch_remember(args: &str, _ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    let cwd = env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    DispatchResult::Handled {
        output: remember_output_lines(args.trim(), &cwd),
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

pub(super) fn dispatch_memories(args: &str, _ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    let cwd = env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    DispatchResult::Handled {
        output: memories_output_lines(args.trim(), &cwd),
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

pub(super) fn dispatch_forget(args: &str, _ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    let cwd = env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    DispatchResult::Handled {
        output: forget_output_lines(args.trim(), &cwd),
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}
