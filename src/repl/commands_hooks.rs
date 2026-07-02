//! `/hooks` — list configured shell hooks (yoyo parity).

use {
    super::dispatch::{DispatchResult, ReplDispatchCtx},
    crate::agents::hooks::hooks_output_lines,
};

pub(super) fn dispatch_hooks(ctx: &ReplDispatchCtx<'_>) -> DispatchResult {
    let shell_hooks = if ctx.bare {
        Vec::new()
    } else {
        ctx.config.get_shell_hooks()
    };
    DispatchResult::Handled {
        output: hooks_output_lines(&shell_hooks),
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}
