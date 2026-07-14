//! `/run` and `/cd` slash handlers (yoyo shell family v1).

use super::{
    dispatch::{DispatchResult, ReplDispatchCtx},
    shell_run::{
        cd_success_lines, change_directory, current_directory_display, format_run_output_lines,
        run_shell_command, run_usage_lines,
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

    let result = run_shell_command(cmd);
    if result.success {
        ctx.session.last_failed_run = None;
    } else {
        ctx.session.last_failed_run = Some(result.clone());
    }
    let (output, detail) = format_run_output_lines(&result);
    DispatchResult::Handled {
        output,
        detail,
        redraw_prompt: true,
        reinstall: None,
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
