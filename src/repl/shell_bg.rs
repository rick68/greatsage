//! Background job tracker for `/bg` (yoyo parity; no LLM).
//!
//! Jobs use `tokio::process` on the **app** Tokio runtime owned by
//! `bevy_tokio_tasks::TokioTasksRuntime` (pass `runtime()` from dispatch).
//! Do not create a second production Runtime. Nested pipe drains use a cloned
//! [`Handle`]. Output is capture-only (no live paint) with a 256 KiB cap;
//! Unix process groups enable `/bg kill` on the whole tree.

use std::{
    collections::HashMap,
    process::{ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    runtime::{Handle, Runtime},
    task::JoinHandle,
};

/// Maximum bytes of output to buffer per background job (yoyo / StreamingBashTool).
pub(crate) const MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// Default number of tail lines shown by `/bg output`.
pub(crate) const DEFAULT_TAIL_LINES: usize = 50;

/// Prefix on `/bg output` display lines so the terminal can paint white flush-left
/// without mistaking job body text for `/run` exit rows.
pub(crate) const BG_OUTPUT_BODY_PREFIX: &str = "\u{200B}\u{200B}BGOUT\u{200B}";

/// Snapshot of a job for `/bg list` (no Arc/Mutex).
#[derive(Debug, Clone)]
pub(crate) struct JobSnapshot {
    pub id: u32,
    pub command: String,
    pub finished: bool,
    pub exit_code: Option<i32>,
    pub elapsed: Duration,
}

struct BackgroundJob {
    id: u32,
    command: String,
    started_at: Instant,
    output: Arc<Mutex<String>>,
    finished: Arc<AtomicBool>,
    exit_code: Arc<Mutex<Option<i32>>>,
    /// Child pid (process group leader on Unix) for kill; `None` after reaped.
    pid: Arc<Mutex<Option<u32>>>,
    /// Tokio task join handle (dropped when killed or finished).
    worker: Mutex<Option<JoinHandle<()>>>,
}

/// Tracks process-scoped background shell jobs.
#[derive(Clone)]
pub(crate) struct BackgroundJobTracker {
    jobs: Arc<Mutex<HashMap<u32, Arc<BackgroundJob>>>>,
    next_id: Arc<AtomicU32>,
}

impl Default for BackgroundJobTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundJobTracker {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU32::new(1)),
        }
    }

    /// Spawn `sh -c <command>` on the **app** Tokio runtime. Returns the job id.
    ///
    /// `runtime` must be `TokioTasksRuntime::runtime()` from the Bevy app (or a
    /// test-only Runtime). Nested pipe tasks use a cloned [`Handle`].
    pub fn launch(&self, command: &str, runtime: &Runtime) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let output = Arc::new(Mutex::new(String::new()));
        let finished = Arc::new(AtomicBool::new(false));
        let exit_code = Arc::new(Mutex::new(None));
        let pid = Arc::new(Mutex::new(None));

        let job = Arc::new(BackgroundJob {
            id,
            command: command.to_string(),
            started_at: Instant::now(),
            output: Arc::clone(&output),
            finished: Arc::clone(&finished),
            exit_code: Arc::clone(&exit_code),
            pid: Arc::clone(&pid),
            worker: Mutex::new(None),
        });

        let cmd_string = command.to_string();
        let out = Arc::clone(&output);
        let fin = Arc::clone(&finished);
        let code = Arc::clone(&exit_code);
        let pid_slot = Arc::clone(&pid);

        let handle = runtime.handle().clone();
        let join = runtime.spawn(async move {
            run_background_command(&cmd_string, out, fin, code, pid_slot, handle).await;
        });

        if let Ok(mut w) = job.worker.lock() {
            *w = Some(join);
        }

        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.insert(id, job);
        }

        id
    }

    pub fn list(&self) -> Vec<JobSnapshot> {
        let Ok(jobs) = self.jobs.lock() else {
            return Vec::new();
        };
        let mut snapshots: Vec<JobSnapshot> = jobs
            .values()
            .map(|j| JobSnapshot {
                id: j.id,
                command: j.command.clone(),
                finished: j.finished.load(Ordering::Relaxed),
                exit_code: j.exit_code.lock().ok().and_then(|g| *g),
                elapsed: j.started_at.elapsed(),
            })
            .collect();
        snapshots.sort_by_key(|s| s.id);
        snapshots
    }

    pub fn get_output(&self, id: u32) -> Option<String> {
        let jobs = self.jobs.lock().ok()?;
        let job = jobs.get(&id)?;
        let guard = job.output.lock().ok()?;
        Some(guard.clone())
    }

    pub fn exists(&self, id: u32) -> bool {
        self.jobs
            .lock()
            .map(|j| j.contains_key(&id))
            .unwrap_or(false)
    }

    #[allow(dead_code)] // used by unit tests + future status lines
    pub fn is_finished(&self, id: u32) -> bool {
        self.jobs
            .lock()
            .ok()
            .and_then(|j| j.get(&id).map(|job| job.finished.load(Ordering::Relaxed)))
            .unwrap_or(false)
    }

    /// Kill a still-running job. Returns:
    /// - `Ok(true)` killed
    /// - `Ok(false)` already finished
    /// - `Err(())` unknown id
    pub fn kill(&self, id: u32) -> Result<bool, ()> {
        let job = {
            let jobs = self.jobs.lock().map_err(|_| ())?;
            jobs.get(&id).cloned().ok_or(())?
        };

        if job.finished.load(Ordering::Relaxed) {
            return Ok(false);
        }

        let child_pid = job.pid.lock().ok().and_then(|g| *g);
        if let Some(pid) = child_pid {
            signal_process_group(pid, true);
            std::thread::sleep(Duration::from_millis(50));
            // If still not finished, hard kill.
            if !job.finished.load(Ordering::Relaxed) {
                signal_process_group(pid, false);
            }
        }

        // Wait briefly for worker to reap; if stuck, force finished state.
        for _ in 0..20 {
            if job.finished.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        if !job.finished.load(Ordering::Relaxed) {
            job.finished.store(true, Ordering::Relaxed);
            if let Ok(mut code) = job.exit_code.lock() {
                if code.is_none() {
                    *code = Some(-1);
                }
            }
        }

        // Drop task handle (task should exit after child reaped).
        if let Ok(mut w) = job.worker.lock() {
            let _ = w.take();
        }

        Ok(true)
    }
}

fn exit_code_from_status(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    -1
}

fn signal_process_group(pid: u32, soft: bool) {
    #[cfg(unix)]
    {
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

fn append_capped(buf: &mut String, chunk: &str) {
    if chunk.is_empty() {
        return;
    }
    let remaining = MAX_OUTPUT_BYTES.saturating_sub(buf.len());
    if remaining == 0 {
        return;
    }
    if chunk.len() <= remaining {
        buf.push_str(chunk);
        return;
    }
    // Prefer char boundary for UTF-8 safety.
    let mut end = remaining;
    while end > 0 && !chunk.is_char_boundary(end) {
        end -= 1;
    }
    buf.push_str(&chunk[..end]);
}

async fn drain_pipe_capped(mut pipe: impl AsyncRead + Unpin, output: Arc<Mutex<String>>) {
    let mut buf = [0u8; 4096];
    loop {
        match pipe.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                if let Ok(mut out) = output.lock() {
                    append_capped(&mut out, &text);
                }
            }
            Err(_) => break,
        }
    }
}

async fn run_background_command(
    command: &str,
    output: Arc<Mutex<String>>,
    finished: Arc<AtomicBool>,
    exit_code: Arc<Mutex<Option<i32>>>,
    pid_slot: Arc<Mutex<Option<u32>>>,
    handle: Handle,
) {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.as_std_mut().process_group(0);
    }

    // Must run inside a task on the app Runtime (reactor for tokio::process).
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            if let Ok(mut out) = output.lock() {
                append_capped(&mut out, &format!("Failed to spawn: {e}\n"));
            }
            finished.store(true, Ordering::Relaxed);
            if let Ok(mut code) = exit_code.lock() {
                *code = Some(-1);
            }
            return;
        }
    };

    if let Ok(mut p) = pid_slot.lock() {
        *p = child.id();
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let out_stdout = Arc::clone(&output);
    let out_stderr = Arc::clone(&output);
    let stdout_h = stdout.map(|p| {
        handle.spawn(async move {
            () = drain_pipe_capped(p, out_stdout).await;
        })
    });
    let stderr_h = stderr.map(|p| {
        handle.spawn(async move {
            () = drain_pipe_capped(p, out_stderr).await;
        })
    });

    let status = child.wait().await;
    if let Some(h) = stdout_h {
        let _ = h.await;
    }
    if let Some(h) = stderr_h {
        let _ = h.await;
    }

    let code = match status {
        Ok(s) => exit_code_from_status(s),
        Err(_) => -1,
    };
    if let Ok(mut slot) = exit_code.lock() {
        *slot = Some(code);
    }
    if let Ok(mut p) = pid_slot.lock() {
        *p = None;
    }
    () = finished.store(true, Ordering::Relaxed);
}

/// Format elapsed duration for display (yoyo-style).
pub(crate) fn format_elapsed(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Truncate a command string for display (first line only).
pub(crate) fn truncate_command(cmd: &str, max: usize) -> String {
    let cmd = cmd.lines().next().unwrap_or(cmd);
    if cmd.chars().count() <= max {
        return String::from(cmd);
    }
    let mut out: String = cmd.chars().take(max.saturating_sub(1)).collect();
    () = out.push('…');
    out
}

/// Tail the last `n` lines of a string. Returns `(body, omitted_line_count)`.
pub(crate) fn tail_lines_with_omitted(s: &str, n: usize) -> (String, usize) {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= n {
        return (s.to_string(), 0);
    }
    let omitted = lines.len() - n;
    let body = lines[lines.len() - n..].join("\n");
    // Preserve trailing newline if original ended with one and tail is non-empty.
    let body = if s.ends_with('\n') {
        format!("{body}\n")
    } else {
        body
    };
    (body, omitted)
}

pub(crate) fn bg_usage_lines() -> Vec<String> {
    vec!["Usage: /bg run <cmd> | /bg list | /bg output <id> | /bg kill <id>".to_string()]
}

pub(crate) fn bg_run_usage_lines() -> Vec<String> {
    vec!["Usage: /bg run <command>".to_string()]
}

pub(crate) fn bg_started_line(id: u32, command: &str) -> String {
    format!(
        "⚡ Background job [{id}] started: {}",
        truncate_command(command, 60)
    )
}

pub(crate) fn format_list_lines(jobs: &[JobSnapshot]) -> Vec<String> {
    if jobs.is_empty() {
        return vec!["No background jobs".to_string()];
    }
    let mut lines = vec!["Background Jobs".to_string()];
    for job in jobs {
        let status = if job.finished {
            match job.exit_code {
                Some(0) => "✓ done".to_string(),
                Some(code) => format!("✗ exit {code}"),
                None => "✗ done".to_string(),
            }
        } else {
            String::from("● running")
        };
        let elapsed = format_elapsed(job.elapsed);
        let cmd = truncate_command(&job.command, 50);
        () = lines.push(format!("  [{}]  {status}  {elapsed}  {cmd}", job.id));
    }
    lines
}

pub(crate) fn mark_bg_output_line(line: impl AsRef<str>) -> String {
    format!("{BG_OUTPUT_BODY_PREFIX}{}", line.as_ref())
}

pub(crate) fn format_output_lines(output: &str, show_all: bool) -> Vec<String> {
    if output.is_empty() {
        return vec![mark_bg_output_line("(no output yet)")];
    }
    if show_all {
        // Split into lines for Handled output vec (preserve empty trailing).
        let mut lines: Vec<String> = output.lines().map(mark_bg_output_line).collect();
        if output.ends_with('\n') {
            // lines() drops final empty; keep content as-is via join later in tests —
            // for display, line list without forced trailing empty is fine.
        }
        if lines.is_empty() {
            () = lines.push(mark_bg_output_line(output));
        }
        return lines;
    }
    let (tail, omitted) = tail_lines_with_omitted(output, DEFAULT_TAIL_LINES);
    let mut lines = Vec::new();
    if omitted > 0 {
        () = lines.push(mark_bg_output_line(format!(
            "... ({omitted} lines omitted, use --all to see everything)"
        )));
    }
    for line in tail.lines() {
        () = lines.push(mark_bg_output_line(line));
    }
    if lines.is_empty() {
        () = lines.push(mark_bg_output_line(tail));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn test_runtime() -> &'static tokio::runtime::Runtime {
        static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("bg test runtime")
        })
    }

    #[test]
    fn launch_list_echo() {
        let tracker = BackgroundJobTracker::new();
        let id = tracker.launch("echo hello-bg", test_runtime());
        assert_eq!(id, 1);

        for _ in 0..40 {
            if tracker.is_finished(id) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(tracker.is_finished(id), "echo should finish quickly");

        let jobs = tracker.list();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, 1);
        assert!(jobs[0].finished);
        assert_eq!(jobs[0].exit_code, Some(0));

        let out = tracker.get_output(id).expect("output");
        assert!(out.contains("hello-bg"), "out={out:?}");
    }

    #[test]
    fn tail_lines_omits() {
        let mut s = String::new();
        for i in 0..60 {
            s.push_str(&format!("line{i}\n"));
        }
        let (tail, omitted) = tail_lines_with_omitted(&s, 50);
        assert_eq!(omitted, 10);
        assert!(tail.contains("line59"));
        assert!(!tail.contains("line0\n") || !tail.starts_with("line0"));
        assert!(!tail.lines().any(|l| l == "line0"));
    }

    #[test]
    fn append_capped_bounds() {
        let mut buf = String::new();
        let chunk = "x".repeat(MAX_OUTPUT_BYTES + 100);
        append_capped(&mut buf, &chunk);
        assert_eq!(buf.len(), MAX_OUTPUT_BYTES);
        append_capped(&mut buf, "more");
        assert_eq!(buf.len(), MAX_OUTPUT_BYTES);
    }

    #[test]
    fn kill_sleep() {
        let tracker = BackgroundJobTracker::new();
        let id = tracker.launch("sleep 30", test_runtime());
        std::thread::sleep(Duration::from_millis(100));
        assert!(!tracker.is_finished(id));
        assert_eq!(tracker.kill(id), Ok(true));
        for _ in 0..40 {
            if tracker.is_finished(id) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(tracker.is_finished(id));
        assert_eq!(tracker.kill(id), Ok(false)); // already finished
        assert_eq!(tracker.kill(999), Err(()));
    }

    #[test]
    fn multiple_ids() {
        let tracker = BackgroundJobTracker::new();
        let rt = test_runtime();
        let a = tracker.launch("echo a", rt);
        let b = tracker.launch("echo b", rt);
        assert_eq!(a, 1);
        assert_eq!(b, 2);
        for _ in 0..40 {
            if tracker.is_finished(a) && tracker.is_finished(b) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(tracker.list().len(), 2);
    }
}
