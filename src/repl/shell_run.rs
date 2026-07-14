//! Shell helpers for `/run`, `!`, and `/cd` (yoyo parity; no LLM).
//!
//! Active runs use a worker thread so the Bevy schedule is not frozen on
//! `Command::output()`. Unix children run in their own process group for SIGINT.

use std::{
    env,
    io::Read,
    path::PathBuf,
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Result of running a shell command via `/run` or `!`.
#[derive(Debug, Clone)]
pub(crate) struct RunResult {
    /// Original command string (kept for future `/fix`).
    #[allow(dead_code)]
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
    pub success: bool,
    /// Operator closed piped stdin with Ctrl+D (EOF) before the child exited.
    pub stdin_eof: bool,
}

/// Process-scoped snapshot stored for a future `/fix` change.
pub(crate) type LastFailedRun = RunResult;

/// Control messages for an in-flight shell worker.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ShellCtrl {
    SoftInterrupt,
    HardInterrupt,
    CloseStdin,
}

/// Handle for a non-blocking shell run (Send + Sync for `ReplSessionState`).
pub(crate) struct ActiveShellHandle {
    #[allow(dead_code)] // kept for future status lines / `/fix` context
    pub command: String,
    pub result_rx: crossbeam_channel::Receiver<RunResult>,
    pub ctrl_tx: crossbeam_channel::Sender<ShellCtrl>,
    /// True after the first soft interrupt was requested (second Ctrl+C → hard).
    pub soft_sent: bool,
}

/// Parse a bang line after optional leading whitespace.
///
/// - `None` — not a bang line
/// - `Some("")` — bare `!` / `!   ` (usage)
/// - `Some(body)` — command body (`!!` → `"!"`)
pub(crate) fn parse_bang_command(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix('!')?;
    Some(rest.trim())
}

/// Live child for `/run` / `!` (owned by the worker thread).
struct ShellChild {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout_reader: Option<JoinHandle<Vec<u8>>>,
    stderr_reader: Option<JoinHandle<Vec<u8>>>,
    command: String,
    start: Instant,
    /// Set when the operator sent Ctrl+D (close stdin write end).
    stdin_eof: bool,
}

fn exit_code_from_status(status: ExitStatus) -> (i32, bool) {
    if let Some(code) = status.code() {
        return (code, status.success());
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return (128 + sig, false);
        }
    }
    (-1, false)
}

fn signal_process_group(pid: u32, soft: bool) {
    #[cfg(unix)]
    {
        // process_group(0) ⇒ child's pid is the process-group id.
        let arg = if soft { "-INT" } else { "-KILL" };
        let _ = Command::new("kill")
            .args([arg, &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, soft);
    }
}

fn spawn_reader(mut pipe: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    })
}

/// Spawn `sh -c <cmd>` with piped stdio and (Unix) own process group.
fn spawn_shell_child(cmd: &str) -> Result<ShellChild, String> {
    let mut command = Command::new("sh");
    command
        .args(["-c", cmd])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = command
        .spawn()
        .map_err(|e| format!("error running command: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "missing stdout pipe".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "missing stderr pipe".to_string())?;
    let stdin = child.stdin.take();

    Ok(ShellChild {
        child,
        stdin,
        stdout_reader: Some(spawn_reader(stdout)),
        stderr_reader: Some(spawn_reader(stderr)),
        command: cmd.to_string(),
        start: Instant::now(),
        stdin_eof: false,
    })
}

fn apply_ctrl(shell: &mut ShellChild, ctrl: ShellCtrl) {
    match ctrl {
        ShellCtrl::SoftInterrupt => {
            let pid = shell.child.id();
            signal_process_group(pid, true);
            #[cfg(not(unix))]
            {
                let _ = shell.child.kill();
            }
        }
        ShellCtrl::HardInterrupt => {
            let pid = shell.child.id();
            signal_process_group(pid, false);
            let _ = shell.child.kill();
        }
        ShellCtrl::CloseStdin => {
            shell.stdin.take();
            shell.stdin_eof = true;
        }
    }
}

fn join_pipe(handle: Option<JoinHandle<Vec<u8>>>) -> String {
    handle
        .and_then(|h| h.join().ok())
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}

fn try_poll_shell(shell: &mut ShellChild) -> Option<RunResult> {
    match shell.child.try_wait() {
        Ok(Some(status)) => {
            shell.stdin.take();
            let stdout = join_pipe(shell.stdout_reader.take());
            let stderr = join_pipe(shell.stderr_reader.take());
            let (exit_code, success) = exit_code_from_status(status);
            Some(RunResult {
                command: shell.command.clone(),
                exit_code,
                stdout,
                stderr,
                elapsed: shell.start.elapsed(),
                success,
                stdin_eof: shell.stdin_eof,
            })
        }
        Ok(None) => None,
        Err(e) => {
            shell.stdin.take();
            let stdout = join_pipe(shell.stdout_reader.take());
            let mut stderr = join_pipe(shell.stderr_reader.take());
            if stderr.is_empty() {
                stderr = format!("error waiting for command: {e}");
            } else {
                stderr = format!("{stderr}\nerror waiting for command: {e}");
            }
            Some(RunResult {
                command: shell.command.clone(),
                exit_code: -1,
                stdout,
                stderr,
                elapsed: shell.start.elapsed(),
                success: false,
                stdin_eof: shell.stdin_eof,
            })
        }
    }
}

/// Start a shell run on a worker thread. Returns a handle for poll / interrupt.
pub(crate) fn start_shell_run(cmd: &str) -> Result<ActiveShellHandle, String> {
    let (result_tx, result_rx) = crossbeam_channel::bounded(1);
    let (ctrl_tx, ctrl_rx) = crossbeam_channel::unbounded();
    let command = cmd.to_string();
    let command_for_handle = command.clone();

    let _worker = thread::spawn(move || {
        let mut shell = match spawn_shell_child(&command) {
            Ok(s) => s,
            Err(e) => {
                let _ = result_tx.send(RunResult {
                    command,
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: e,
                    elapsed: Duration::ZERO,
                    success: false,
                    stdin_eof: false,
                });
                return;
            }
        };

        loop {
            while let Ok(ctrl) = ctrl_rx.try_recv() {
                apply_ctrl(&mut shell, ctrl);
            }
            if let Some(result) = try_poll_shell(&mut shell) {
                let _ = result_tx.send(result);
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
    });

    Ok(ActiveShellHandle {
        command: command_for_handle,
        result_rx,
        ctrl_tx,
        soft_sent: false,
    })
}

/// Request interrupt: first soft (SIGINT group), second hard (SIGKILL).
pub(crate) fn request_shell_interrupt(handle: &mut ActiveShellHandle) {
    if handle.soft_sent {
        let _ = handle.ctrl_tx.send(ShellCtrl::HardInterrupt);
    } else {
        handle.soft_sent = true;
        let _ = handle.ctrl_tx.send(ShellCtrl::SoftInterrupt);
    }
}

/// Close piped stdin (Ctrl+D / EOF).
pub(crate) fn request_shell_stdin_eof(handle: &ActiveShellHandle) {
    let _ = handle.ctrl_tx.send(ShellCtrl::CloseStdin);
}

/// Non-blocking: take finished result if ready.
pub(crate) fn try_recv_shell_result(handle: &ActiveShellHandle) -> Option<RunResult> {
    handle.result_rx.try_recv().ok()
}

/// Blocking convenience for unit tests and simple callers (waits until done).
#[allow(dead_code)] // used by `#[cfg(test)]` and devenv structural parity
pub(crate) fn run_shell_command(cmd: &str) -> RunResult {
    match start_shell_run(cmd) {
        Ok(handle) => loop {
            if let Some(r) = try_recv_shell_result(&handle) {
                return r;
            }
            thread::sleep(Duration::from_millis(5));
        },
        Err(e) => RunResult {
            command: cmd.to_string(),
            exit_code: -1,
            stdout: String::new(),
            stderr: e,
            elapsed: Duration::ZERO,
            success: false,
            stdin_eof: false,
        },
    }
}

/// Run + interrupt after a short delay (unit tests).
#[cfg(test)]
pub(crate) fn run_shell_command_interrupt_after(cmd: &str, delay: Duration) -> RunResult {
    let mut handle = start_shell_run(cmd).expect("spawn");
    thread::sleep(delay);
    request_shell_interrupt(&mut handle);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(r) = try_recv_shell_result(&handle) {
            return r;
        }
        if Instant::now() > deadline {
            request_shell_interrupt(&mut handle); // hard
        }
        if Instant::now() > deadline + Duration::from_secs(2) {
            return RunResult {
                command: cmd.to_string(),
                exit_code: -1,
                stdout: String::new(),
                stderr: "interrupt test timed out".into(),
                elapsed: Duration::from_secs(7),
                success: false,
                stdin_eof: false,
            };
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(crate) fn format_run_duration(elapsed: Duration) -> String {
    let ms = elapsed.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", elapsed.as_secs_f64())
    }
}

/// Usage lines for bare `/run` or bare `!` (yoyo-style consecutive output).
pub(crate) fn run_usage_lines() -> Vec<String> {
    vec![
        "usage: /run <command>  or  !<command>".to_string(),
        "Runs a shell command directly (no AI, no tokens).".to_string(),
    ]
}

pub(crate) fn shell_busy_lines() -> Vec<String> {
    vec!["a shell command is already running (Ctrl+C to interrupt)".to_string()]
}

/// Max lines for yoyo-style failure re-preview under `✗ exit` (before 💡 tip).
const RUN_FAIL_PREVIEW_MAX: usize = 3;

/// Leading spaces for failure re-preview lines (yoyo `print_run_result`: 4 spaces).
const RUN_FAIL_PREVIEW_INDENT: &str = "    ";

fn is_error_like_stdout_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("error") || lower.contains("failed") || lower.contains("panic")
}

/// yoyo: up to 3 stderr lines, else up to 3 error-like stdout lines; each with 4-space indent.
fn failure_preview_lines(result: &RunResult) -> Vec<String> {
    let mut preview: Vec<String> = result
        .stderr
        .lines()
        .take(RUN_FAIL_PREVIEW_MAX)
        .map(|l| format!("{RUN_FAIL_PREVIEW_INDENT}{l}"))
        .collect();
    if preview.is_empty() {
        preview = result
            .stdout
            .lines()
            .filter(|l| is_error_like_stdout_line(l))
            .take(RUN_FAIL_PREVIEW_MAX)
            .map(|l| format!("{RUN_FAIL_PREVIEW_INDENT}{l}"))
            .collect();
    }
    preview
}

/// Prefix on body lines when the run saw Ctrl+D (stdin EOF).
/// Stripped in `style_run_line`; body painted red; exit stays gray/`✗` red.
pub(crate) const RUN_STDIN_EOF_BODY_PREFIX: &str = "\u{200B}\u{200B}STDIN_EOF\u{200B}";

/// Lines to print for a completed run.
///
/// Order: stdout → stderr → `✓`/`✗ exit` → (fail: re-preview ≤3) → (fail: 💡 tip).
/// No blank line before exit. Preview uses 4-space indent (yoyo).
///
/// After Ctrl+D (`stdin_eof`): body lines before exit are marked for **red**;
/// `✓ exit` stays dim gray.
pub(crate) fn format_run_output_lines(result: &RunResult) -> (Vec<String>, Vec<String>) {
    let mut output: Vec<String> = result.stdout.lines().map(str::to_string).collect();
    () = output.extend(result.stderr.lines().map(str::to_string));

    if result.stdin_eof {
        for line in &mut output {
            if !line.is_empty() {
                *line = format!("{RUN_STDIN_EOF_BODY_PREFIX}{line}");
            }
        }
    }

    let elapsed = format_run_duration(result.elapsed);
    if result.success {
        () = output.push(format!("✓ exit {} ({elapsed})", result.exit_code));
    } else {
        () = output.push(format!("✗ exit {} ({elapsed})", result.exit_code));
        () = output.extend(failure_preview_lines(result));
        () = output.push(
            "💡 Command failed. Ask me to analyze the error, or say /fix to auto-fix.".to_string(),
        );
    }
    (output, Vec::new())
}

pub(crate) fn current_directory_display() -> String {
    env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string())
}

/// yoyo note after a successful `/cd <path>` (project context not reloaded).
pub(crate) const CD_CONTEXT_NOTE: &str = "(project context was loaded from the original \
directory and is not reloaded — use /context to review)";

/// Success lines for `/cd <path>`: path (white in terminal) + dim context note.
pub(crate) fn cd_success_lines(new_cwd: &std::path::Path) -> Vec<String> {
    vec![new_cwd.display().to_string(), CD_CONTEXT_NOTE.to_string()]
}

/// Expand `~` / `~/…` and resolve relative paths against the process cwd.
pub(crate) fn expand_cd_path(raw: &str) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("empty path".to_string());
    }

    let expanded = if raw == "~" {
        home_dir().ok_or_else(|| "HOME is not set".to_string())?
    } else if let Some(rest) = raw.strip_prefix("~/") {
        let home = home_dir().ok_or_else(|| "HOME is not set".to_string())?;
        home.join(rest)
    } else {
        PathBuf::from(raw)
    };

    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        let cwd = env::current_dir().map_err(|e| format!("cannot read cwd: {e}"))?;
        Ok(cwd.join(expanded))
    }
}

pub(crate) fn change_directory(raw: &str) -> Result<PathBuf, String> {
    let path = expand_cd_path(raw)?;
    if !path.exists() {
        return Err(format!("no such file or directory: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("not a directory: {}", path.display()));
    }
    () =
        env::set_current_dir(&path).map_err(|e| format!("cannot cd to {}: {e}", path.display()))?;
    env::current_dir().map_err(|e| format!("cd succeeded but cannot read cwd: {e}"))
}

fn home_dir() -> Option<PathBuf> {
    dirs::home_dir().or_else(|| env::var_os("HOME").map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bang_basic() {
        assert_eq!(parse_bang_command("!ls -la"), Some("ls -la"));
        assert_eq!(parse_bang_command("  !git status"), Some("git status"));
        assert_eq!(parse_bang_command("!echo hi  "), Some("echo hi"));
    }

    #[test]
    fn parse_bang_empty_and_double() {
        assert_eq!(parse_bang_command("!"), Some(""));
        assert_eq!(parse_bang_command("!   "), Some(""));
        assert_eq!(parse_bang_command("!!"), Some("!"));
    }

    #[test]
    fn parse_bang_not_bang() {
        assert_eq!(parse_bang_command("hello"), None);
        assert_eq!(parse_bang_command("/help"), None);
        assert_eq!(parse_bang_command("wow!"), None);
    }

    #[test]
    fn run_shell_echo() {
        let r = run_shell_command("echo hello-shell");
        assert!(r.success, "stderr={}", r.stderr);
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello-shell"));
    }

    #[test]
    fn run_shell_nonzero() {
        let r = run_shell_command("false");
        assert!(!r.success);
        assert_ne!(r.exit_code, 0);
    }

    #[test]
    fn run_shell_interrupt_sleep() {
        let r = run_shell_command_interrupt_after("sleep 30", Duration::from_millis(150));
        assert!(!r.success, "interrupted sleep should fail: {r:?}");
        assert_ne!(r.exit_code, 0);
        assert!(
            r.elapsed < Duration::from_secs(10),
            "elapsed {:?}",
            r.elapsed
        );
    }

    #[test]
    fn format_run_puts_exit_after_stdout_and_stderr() {
        let r = RunResult {
            command: "x".into(),
            exit_code: 1,
            stdout: "out\n".into(),
            stderr: "err\n".into(),
            elapsed: Duration::from_millis(1),
            success: false,
            stdin_eof: false,
        };
        let (lines, detail) = format_run_output_lines(&r);
        assert!(detail.is_empty(), "exit must not use detail channel");
        let exit_i = lines
            .iter()
            .position(|l| l.starts_with("✗ exit "))
            .expect("exit line");
        let out_i = lines.iter().position(|l| l == "out").unwrap();
        let err_i = lines.iter().position(|l| l == "err").unwrap();
        assert!(
            out_i < err_i && err_i < exit_i,
            "order: stdout, stderr, exit — {lines:?}"
        );
        assert_eq!(exit_i, err_i + 1, "no blank line before exit: {lines:?}");
    }

    #[test]
    fn format_run_failure_preview_before_tip() {
        let r = RunResult {
            command: "ps".into(),
            exit_code: 1,
            stdout: String::new(),
            stderr: "ps: illegal option -- -\nusage: ps [-AaCcEefhjlMmrSTvwXx]\n          [-g grp]\n       ps [-L]\n".into(),
            elapsed: Duration::from_millis(10),
            success: false,
            stdin_eof: false,
        };
        let (lines, _) = format_run_output_lines(&r);
        let exit_i = lines.iter().position(|l| l.starts_with("✗ exit ")).unwrap();
        let tip_i = lines
            .iter()
            .position(|l| l.starts_with("💡 Command failed."))
            .expect("tip");
        let preview: Vec<&String> = lines[exit_i + 1..tip_i].iter().collect();
        assert_eq!(
            preview.len(),
            3,
            "yoyo takes up to 3 preview lines: {lines:?}"
        );
        assert!(
            preview.iter().all(|l| l.starts_with("    ")),
            "preview lines use 4-space indent: {preview:?}"
        );
        assert!(preview[0].contains("illegal option"));
        assert_eq!(tip_i, exit_i + 1 + preview.len());
    }

    #[test]
    fn expand_tilde_home() {
        if home_dir().is_none() {
            return;
        }
        let p = expand_cd_path("~").expect("expand ~");
        assert_eq!(p, home_dir().unwrap());
    }

    #[test]
    fn format_duration_units() {
        assert_eq!(format_run_duration(Duration::from_millis(42)), "42ms");
        assert_eq!(format_run_duration(Duration::from_millis(1500)), "1.5s");
    }

    #[test]
    fn format_run_stdin_eof_marks_body_red_not_exit() {
        let r = RunResult {
            command: "cat".into(),
            exit_code: 0,
            stdout: "hello from cat\n".into(),
            stderr: String::new(),
            elapsed: Duration::from_millis(3),
            success: true,
            stdin_eof: true,
        };
        let (lines, _) = format_run_output_lines(&r);
        let body = lines
            .iter()
            .find(|l| l.contains("hello from cat"))
            .expect("body");
        assert!(
            body.starts_with(RUN_STDIN_EOF_BODY_PREFIX),
            "Ctrl+D body marked for red styling: {body}"
        );
        let exit = lines
            .iter()
            .find(|l| l.starts_with("✓ exit "))
            .expect("exit");
        assert!(
            !exit.starts_with(RUN_STDIN_EOF_BODY_PREFIX),
            "exit must not get red-body prefix (stays gray): {exit}"
        );
    }
}
