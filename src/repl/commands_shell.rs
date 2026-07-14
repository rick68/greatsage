//! `/run`, `/cd`, and `/bg` slash handlers (yoyo shell family).

use super::{
    dispatch::{DispatchResult, ReplDispatchCtx},
    shell_bg::{
        bg_run_usage_lines, bg_started_line, bg_usage_lines, format_list_lines,
        format_output_lines, mark_bg_output_line,
    },
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

    // App Runtime from TokioTasksRuntime (dispatch); do not Runtime::new().
    match start_shell_run(cmd, ctx.runtime) {
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

/// `/bg [run|list|output|kill …]` — background jobs (independent of `active_shell`).
pub(super) fn dispatch_bg(args: &str, ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    let input = args.trim();
    let (sub, rest) = match input.find(char::is_whitespace) {
        Some(pos) => (&input[..pos], input[pos..].trim()),
        None => {
            if input.is_empty() {
                ("list", "")
            } else {
                (input, "")
            }
        }
    };

    match sub {
        "run" => dispatch_bg_run(rest, ctx),
        "list" => dispatch_bg_list(ctx),
        "output" => dispatch_bg_output(rest, ctx),
        "kill" => dispatch_bg_kill(rest, ctx),
        _ => DispatchResult::Handled {
            output: {
                let mut lines = vec![format!("Unknown /bg subcommand: {sub}")];
                () = lines.extend(bg_usage_lines());
                lines
            },
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
    }
}

fn dispatch_bg_run(command: &str, ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    let command = command.trim();
    if command.is_empty() {
        return DispatchResult::Handled {
            output: bg_run_usage_lines(),
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        };
    }
    let id = ctx.session.bg_jobs.launch(command, ctx.runtime);
    DispatchResult::Handled {
        output: vec![bg_started_line(id, command)],
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

fn dispatch_bg_list(ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    let jobs = ctx.session.bg_jobs.list();
    DispatchResult::Handled {
        output: format_list_lines(&jobs),
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

fn dispatch_bg_output(args: &str, ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    let (id_str, flags) = match args.find(char::is_whitespace) {
        Some(pos) => (&args[..pos], args[pos..].trim()),
        None => (args, ""),
    };

    let id = match id_str.parse::<u32>() {
        Ok(id) => id,
        Err(_) => {
            return DispatchResult::Handled {
                output: vec!["Usage: /bg output <id> [--all]".to_string()],
                detail: Vec::new(),
                redraw_prompt: true,
                reinstall: None,
            };
        }
    };

    if !ctx.session.bg_jobs.exists(id) {
        return DispatchResult::Handled {
            output: vec![mark_bg_output_line(format!("No job with ID {id}"))],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        };
    }

    let show_all = flags.contains("--all");
    let output = ctx.session.bg_jobs.get_output(id).unwrap_or_default();
    DispatchResult::Handled {
        output: format_output_lines(&output, show_all),
        detail: Vec::new(),
        redraw_prompt: true,
        reinstall: None,
    }
}

fn dispatch_bg_kill(args: &str, ctx: &mut ReplDispatchCtx<'_>) -> DispatchResult {
    let id_str = args.split_whitespace().next().unwrap_or("");
    let id = match id_str.parse::<u32>() {
        Ok(id) => id,
        Err(_) => {
            return DispatchResult::Handled {
                output: vec!["Usage: /bg kill <id>".to_string()],
                detail: Vec::new(),
                redraw_prompt: true,
                reinstall: None,
            };
        }
    };

    match ctx.session.bg_jobs.kill(id) {
        Ok(true) => DispatchResult::Handled {
            output: vec![format!("Killed job [{id}]")],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
        Ok(false) => DispatchResult::Handled {
            output: vec![format!("Job [{id}] already finished")],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
        Err(()) => DispatchResult::Handled {
            output: vec![format!("No running job with ID {id}")],
            detail: Vec::new(),
            redraw_prompt: true,
            reinstall: None,
        },
    }
}
