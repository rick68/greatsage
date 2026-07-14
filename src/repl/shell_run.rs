//! Shell helpers for `/run`, `!`, and `/cd` (yoyo parity; no LLM).
//!
//! Active runs use `tokio::process` on the **app** Tokio runtime owned by
//! [`bevy_tokio_tasks::TokioTasksRuntime`] (via `dispatch` / `runtime()`).
//! Production code must not build a second Runtime — that conflicts with the
//! plugin (which owns the reactor and may `block_on` on the main thread each
//! tick). Nested I/O tasks use a cloned [`Handle`], never free `tokio::spawn`
//! from a sync Bevy system.
//!
//! Live lines and completion cross the Bevy boundary via crossbeam `try_recv`
//! (sync poll each frame). Unix children use their own process group for kill.

use std::{
    env,
    path::PathBuf,
    process::{ExitStatus, Stdio},
    time::{Duration, Instant},
};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, ChildStdin, Command},
    runtime::Runtime,
    sync::mpsc,
};

/// Result of running a shell command via `/run` or `!`.
#[derive(Debug, Clone)]
pub(crate) struct RunResult {
    /// Original command string (kept for future yoyo-aligned `/fix` / health context).
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

/// Process-scoped snapshot of the last non-zero `/run` / `!` (future `/fix` / health).
pub(crate) type LastFailedRun = RunResult;

/// Control messages for an in-flight shell worker.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ShellCtrl {
    SoftInterrupt,
    HardInterrupt,
    CloseStdin,
}

/// One live line from a running `/run` / `!` (yoyo streams as the child prints).
#[derive(Debug, Clone)]
pub(crate) struct ShellLiveLine {
    pub is_stderr: bool,
    pub text: String,
}

/// Handle for a non-blocking shell run (Send + Sync for `ReplSessionState`).
pub(crate) struct ActiveShellHandle {
    #[allow(dead_code)] // kept for future status lines / `/fix` context
    pub command: String,
    pub result_rx: crossbeam_channel::Receiver<RunResult>,
    /// Line-oriented live output (stdout/stderr) while the child is still running.
    pub live_rx: crossbeam_channel::Receiver<ShellLiveLine>,
    pub ctrl_tx: mpsc::UnboundedSender<ShellCtrl>,
    /// True after the first soft interrupt was requested (second Ctrl+C → hard).
    pub soft_sent: bool,
    /// True once at least one live line was drained to the operator (skip re-print body).
    pub body_streamed: bool,
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
        // Short-lived `kill` helper (std Command is fine; not the shell job itself).
        let arg = if soft { "-INT" } else { "-KILL" };
        let _ = std::process::Command::new("kill")
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

/// Stream pipe line-by-line (yoyo `BufReader` parity), collect full text for `RunResult`.
async fn read_lines_live(
    pipe: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    live_tx: crossbeam_channel::Sender<ShellLiveLine>,
    is_stderr: bool,
) -> String {
    let mut reader = BufReader::new(pipe).lines();
    let mut collected = String::new();
    while let Ok(Some(text)) = reader.next_line().await {
        let _ = live_tx.send(ShellLiveLine {
            is_stderr,
            text: text.clone(),
        });
        if !collected.is_empty() {
            collected.push('\n');
        }
        collected.push_str(&text);
    }
    collected
}

/// Spawn `sh -c <cmd>` with piped stdio and (Unix) own process group.
fn spawn_shell_child(cmd: &str) -> Result<(Child, Option<ChildStdin>), String> {
    let mut command = Command::new("sh");
    command
        .args(["-c", cmd])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }

    let mut child = command
        .spawn()
        .map_err(|e| format!("error running command: {e}"))?;
    let stdin = child.stdin.take();
    Ok((child, stdin))
}

async fn apply_ctrl(
    child: &mut Child,
    stdin: &mut Option<ChildStdin>,
    stdin_eof: &mut bool,
    ctrl: ShellCtrl,
) {
    match ctrl {
        ShellCtrl::SoftInterrupt => {
            if let Some(pid) = child.id() {
                signal_process_group(pid, true);
            }
            #[cfg(not(unix))]
            {
                let _ = child.start_kill();
            }
        }
        ShellCtrl::HardInterrupt => {
            if let Some(pid) = child.id() {
                signal_process_group(pid, false);
            }
            let _ = child.start_kill();
        }
        ShellCtrl::CloseStdin => {
            *stdin = None;
            *stdin_eof = true;
        }
    }
}

/// Start a shell run on the **app** Tokio runtime (`TokioTasksRuntime::runtime()`).
/// Returns a handle for poll / interrupt / live lines.
///
/// `Command::spawn` and pipe reads run only inside the spawned task (reactor
/// required). Nested pipe tasks use the same cloned [`Handle`].
pub(crate) fn start_shell_run(cmd: &str, runtime: &Runtime) -> Result<ActiveShellHandle, String> {
    let (result_tx, result_rx) = crossbeam_channel::bounded(1);
    let (live_tx, live_rx) = crossbeam_channel::unbounded();
    let (ctrl_tx, mut ctrl_rx) = mpsc::unbounded_channel();
    let command = cmd.to_string();
    let command_for_handle = command.clone();

    // Spawn on the app Runtime (owned by bevy_tokio_tasks). Clone Handle only
    // for nested pipe tasks — do not Runtime::new() in production.
    let handle = runtime.handle().clone();
    runtime.spawn(async move {
        let (mut child, mut stdin) = match spawn_shell_child(&command) {
            Ok(pair) => pair,
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

        let start = Instant::now();
        let mut stdin_eof = false;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let live_out = live_tx.clone();
        let live_err = live_tx;
        let stdout_task = handle.spawn(async move {
            match stdout {
                Some(pipe) => read_lines_live(pipe, live_out, false).await,
                None => String::new(),
            }
        });
        let stderr_task = handle.spawn(async move {
            match stderr {
                Some(pipe) => read_lines_live(pipe, live_err, true).await,
                None => String::new(),
            }
        });

        let status = loop {
            tokio::select! {
                ctrl = ctrl_rx.recv() => {
                    match ctrl {
                        Some(c) => apply_ctrl(&mut child, &mut stdin, &mut stdin_eof, c).await,
                        None => {
                            // Handle dropped; keep waiting for the child.
                        }
                    }
                }
                status = child.wait() => {
                    break status;
                }
            }
        };

        // Drop stdin write end so readers can finish if still open.
        drop(stdin);

        let stdout = stdout_task.await.unwrap_or_default();
        let stderr = stderr_task.await.unwrap_or_default();

        let result = match status {
            Ok(status) => {
                let (exit_code, success) = exit_code_from_status(status);
                RunResult {
                    command,
                    exit_code,
                    stdout,
                    stderr,
                    elapsed: start.elapsed(),
                    success,
                    stdin_eof,
                }
            }
            Err(e) => {
                let stderr = if stderr.is_empty() {
                    format!("error waiting for command: {e}")
                } else {
                    format!("{stderr}\nerror waiting for command: {e}")
                };
                RunResult {
                    command,
                    exit_code: -1,
                    stdout,
                    stderr,
                    elapsed: start.elapsed(),
                    success: false,
                    stdin_eof,
                }
            }
        };
        let _ = result_tx.send(result);
    });

    Ok(ActiveShellHandle {
        command: command_for_handle,
        result_rx,
        live_rx,
        ctrl_tx,
        soft_sent: false,
        body_streamed: false,
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

/// Non-blocking: drain one live stdout/stderr line (call in a loop each frame).
pub(crate) fn try_recv_shell_live(handle: &ActiveShellHandle) -> Option<ShellLiveLine> {
    handle.live_rx.try_recv().ok()
}

/// Format a live stream line for `style_run_line` (stderr marker when needed).
pub(crate) fn format_live_stream_line(line: &ShellLiveLine) -> String {
    if line.is_stderr {
        if line.text.is_empty() {
            String::new()
        } else {
            format!("{RUN_STDERR_BODY_PREFIX}{}", line.text)
        }
    } else {
        line.text.clone()
    }
}

/// Blocking convenience for unit tests and simple callers (waits until done).
///
/// Uses a **test-only** Runtime. Production must pass
/// `TokioTasksRuntime::runtime()` from the Bevy app.
#[allow(dead_code)] // used by `#[cfg(test)]` and devenv structural parity
pub(crate) fn run_shell_command(cmd: &str) -> RunResult {
    let runtime = shell_test_runtime();
    match start_shell_run(cmd, runtime) {
        Ok(handle) => loop {
            if let Some(r) = try_recv_shell_result(&handle) {
                return r;
            }
            std::thread::sleep(Duration::from_millis(5));
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

/// Isolated multi-thread runtime for unit tests only (not the Bevy app runtime).
fn shell_test_runtime() -> &'static Runtime {
    use std::sync::OnceLock;
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("shell test runtime")
    })
}

/// Run + interrupt after a short delay (unit tests).
#[cfg(test)]
pub(crate) fn run_shell_command_interrupt_after(cmd: &str, delay: Duration) -> RunResult {
    let runtime = shell_test_runtime();
    let mut handle = start_shell_run(cmd, runtime).expect("spawn");
    std::thread::sleep(delay);
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
        std::thread::sleep(Duration::from_millis(10));
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

/// Prefix on **stderr** body lines for channel-based red styling.
/// Stripped in `style_run_line` (never shown to the operator).
/// Not used when `stdin_eof` already marks the line red via [`RUN_STDIN_EOF_BODY_PREFIX`].
pub(crate) const RUN_STDERR_BODY_PREFIX: &str = "\u{200B}\u{200B}STDERR\u{200B}";

/// Lines to print for a completed run.
///
/// Order: stdout → stderr → `✓`/`✗ exit` → (fail: re-preview ≤3) → (fail: 💡 tip).
/// No blank line before exit. Preview uses 4-space indent (yoyo).
///
/// When `body_already_streamed` is true (live drain while running), body lines are
/// omitted so the operator does not see a duplicate dump after exit — only the
/// footer (exit / re-preview / tip), matching yoyo `print_run_result`.
///
/// Body styling markers (stripped at paint time in `style_run_line`):
/// - stdout: unmarked → white (or [`RUN_STDIN_EOF_BODY_PREFIX`] when Ctrl+D)
/// - stderr: [`RUN_STDERR_BODY_PREFIX`] → red (or EOF prefix alone when Ctrl+D)
/// - `✓ exit` dim / `✗ exit` red (no body markers)
///
/// The 💡 tip still names yoyo's `/fix` (health-driven); that command is **not**
/// shipped in shell polish.
#[allow(dead_code)] // unit tests; production uses `format_run_output_lines_ex` with stream flag
pub(crate) fn format_run_output_lines(result: &RunResult) -> (Vec<String>, Vec<String>) {
    format_run_output_lines_ex(result, false)
}

/// Like [`format_run_output_lines`], with control over body re-print after live stream.
pub(crate) fn format_run_output_lines_ex(
    result: &RunResult,
    body_already_streamed: bool,
) -> (Vec<String>, Vec<String>) {
    let mut output: Vec<String> = if body_already_streamed {
        Vec::new()
    } else {
        let mut body: Vec<String> = result
            .stdout
            .lines()
            .map(|line| mark_stdout_body_line(line, result.stdin_eof))
            .collect();
        () = body.extend(
            result
                .stderr
                .lines()
                .map(|line| mark_stderr_body_line(line, result.stdin_eof)),
        );
        body
    };

    let elapsed = format_run_duration(result.elapsed);
    if result.success {
        () = output.push(format!("✓ exit {} ({elapsed})", result.exit_code));
    } else {
        () = output.push(format!("✗ exit {} ({elapsed})", result.exit_code));
        // yoyo still shows a short re-preview under exit even after streaming.
        () = output.extend(failure_preview_lines(result));
        () = output.push(
            "💡 Command failed. Ask me to analyze the error, or say /fix to auto-fix.".to_string(),
        );
    }
    (output, Vec::new())
}

fn mark_stdout_body_line(line: &str, stdin_eof: bool) -> String {
    if line.is_empty() {
        return String::new();
    }
    if stdin_eof {
        format!("{RUN_STDIN_EOF_BODY_PREFIX}{line}")
    } else {
        line.to_string()
    }
}

fn mark_stderr_body_line(line: &str, stdin_eof: bool) -> String {
    if line.is_empty() {
        return String::new();
    }
    // EOF alone paints red; avoid stacking markers that could leak after one strip.
    if stdin_eof {
        format!("{RUN_STDIN_EOF_BODY_PREFIX}{line}")
    } else {
        format!("{RUN_STDERR_BODY_PREFIX}{line}")
    }
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
    fn live_stream_receives_lines_before_exit() {
        // Two lines with a gap so the live channel is non-empty before join.
        let handle = start_shell_run(
            "printf 'live-a\\n'; sleep 0.15; printf 'live-b\\n'",
            shell_test_runtime(),
        )
        .expect("spawn");
        let mut seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            while let Some(line) = try_recv_shell_live(&handle) {
                seen.push(line.text);
            }
            if let Some(r) = try_recv_shell_result(&handle) {
                while let Some(line) = try_recv_shell_live(&handle) {
                    seen.push(line.text);
                }
                break r;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for shell; seen={seen:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(result.success, "stderr={}", result.stderr);
        assert!(
            seen.iter().any(|l| l == "live-a"),
            "expected live-a in stream: {seen:?}"
        );
        assert!(
            seen.iter().any(|l| l == "live-b"),
            "expected live-b in stream: {seen:?}"
        );
    }

    #[test]
    fn format_run_skips_body_when_already_streamed() {
        let r = RunResult {
            command: "x".into(),
            exit_code: 0,
            stdout: "already shown\n".into(),
            stderr: String::new(),
            elapsed: Duration::from_millis(1),
            success: true,
            stdin_eof: false,
        };
        let (full, _) = format_run_output_lines_ex(&r, false);
        let (footer, _) = format_run_output_lines_ex(&r, true);
        assert!(full.iter().any(|l| l == "already shown"), "{full:?}");
        assert!(
            !footer.iter().any(|l| l.contains("already shown")),
            "footer must not re-print body: {footer:?}"
        );
        assert!(
            footer.iter().any(|l| l.starts_with("✓ exit ")),
            "{footer:?}"
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
        let err_i = lines
            .iter()
            .position(|l| l == &format!("{RUN_STDERR_BODY_PREFIX}err"))
            .unwrap();
        assert!(
            out_i < err_i && err_i < exit_i,
            "order: stdout, stderr, exit — {lines:?}"
        );
        assert_eq!(exit_i, err_i + 1, "no blank line before exit: {lines:?}");
    }

    #[test]
    fn format_run_marks_stderr_not_stdout() {
        let r = RunResult {
            command: "mixed".into(),
            exit_code: 0,
            stdout: "ok\n".into(),
            stderr: "warn\n".into(),
            elapsed: Duration::from_millis(1),
            success: true,
            stdin_eof: false,
        };
        let (lines, _) = format_run_output_lines(&r);
        assert!(
            lines.iter().any(|l| l == "ok"),
            "stdout unmarked: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l == &format!("{RUN_STDERR_BODY_PREFIX}warn")),
            "stderr marked: {lines:?}"
        );
        assert!(
            lines.iter().any(|l| l.starts_with("✓ exit ")),
            "success exit: {lines:?}"
        );
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
