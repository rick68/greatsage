//! `/run` and `/cd` slash handlers (yoyo shell family).

use super::{
    dispatch::{DispatchResult, ReplDispatchCtx},
    shell_run::{
        cd_success_lines, change_directory, current_directory_display, run_usage_lines,
        shell_busy_lines, start_shell_run,
    },
};

pub(super) fn dispatch_run(args: &str, ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    let cmd = args.trim();
    if cmd.is_empty() {
        return DispatchResult::Handled {
            output: run_usage_lines(),
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        };
    }

    if ctx.session.active_shell.is_some() {
        return DispatchResult::Handled {
            output: shell_busy_lines(),
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        };
    }

    match start_shell_run(cmd) {
        Ok(handle) => {
            ctx.session.active_shell = Some(handle);
            // Result is printed by `poll_active_shell_run` when the worker finishes.
            // Redraw prompt only after completion so the line is not stolen mid-run.
            DispatchResult::Handled {
                output: Vec::new(),
                detail: Vec::new(),
                redraw_prompt: false,
                reinstall: None,
            }
        }
        Err(e) => DispatchResult::Handled {
            output: vec![format!("✗ {e}")],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
    }
}

pub(super) fn dispatch_cd(args: &str, _ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    let path = args.trim();
    if path.is_empty() {
        // yoyo: bare `/cd` is pwd — path only (white).
        return DispatchResult::Handled {
            output: vec![current_directory_display()],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        };
    }

    match change_directory(path) {
        Ok(new_cwd) => DispatchResult::Handled {
            // yoyo: path line (white) + dim note that project context is not reloaded.
            output: cd_success_lines(&new_cwd),
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
        Err(err) => DispatchResult::Handled {
            // yoyo-style failure prefix; terminal styles `✗` lines red when detected.
            output: vec![format!("✗ {err}")],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
    }
}
