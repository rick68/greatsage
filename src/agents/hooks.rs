//! yoyo-evolve-style tool pre/post hook pipeline (ADR-0004).

use {
    colored::Colorize,
    std::{
        process::Stdio,
        sync::Arc,
        time::Duration,
    },
    tokio::process::{Child, Command},
    tokio_util::sync::CancellationToken,
    toml_edit::{DocumentMut, Item, Value},
    yoagent::Content,
    yoagent::types::{AgentTool, ToolError, ToolResult},
};

const HOOK_TIMEOUT_SECS: u64 = 5;
/// After `kill()`, wait up to this long to reap the child and avoid zombies.
const HOOK_REAP_TIMEOUT_SECS: u64 = 2;

fn hook_run_timeout() -> Duration {
    #[cfg(test)]
    {
        Duration::from_millis(200)
    }
    #[cfg(not(test))]
    {
        Duration::from_secs(HOOK_TIMEOUT_SECS)
    }
}

/// Hook that runs before/after tool execution.
#[async_trait::async_trait]
pub trait Hook: Send + Sync {
    async fn pre_execute(
        &self,
        _tool_name: &str,
        _params: &serde_json::Value,
        _cancel: &CancellationToken,
    ) -> Result<Option<String>, String> {
        Ok(None)
    }

    async fn post_execute(
        &self,
        _tool_name: &str,
        _params: &serde_json::Value,
        output: &str,
        _cancel: &CancellationToken,
    ) -> Result<String, String> {
        Ok(output.to_string())
    }
}

#[derive(Default)]
pub struct HookRegistry {
    hooks: Vec<Box<dyn Hook>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: vec![] }
    }

    pub fn register(&mut self, hook: Box<dyn Hook>) {
        () = self.hooks.push(hook);
    }

    pub async fn run_pre_hooks(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        cancel: &CancellationToken,
    ) -> Result<Option<String>, String> {
        for hook in &self.hooks {
            match hook.pre_execute(tool_name, params, cancel).await? {
                Some(result) => return Ok(Some(result)),
                None => continue,
            }
        }
        Ok(None)
    }

    pub async fn run_post_hooks(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        output: &str,
        cancel: &CancellationToken,
    ) -> Result<String, String> {
        let mut current = output.to_string();
        for hook in &self.hooks {
            current = hook
                .post_execute(tool_name, params, &current, cancel)
                .await?;
        }
        Ok(current)
    }

    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookPhase {
    Pre,
    Post,
}

#[derive(Clone, Debug)]
pub struct ShellHook {
    pub name: String,
    pub phase: HookPhase,
    pub tool_pattern: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HookRunError {
    Cancelled,
    TimedOut,
    Message(String),
}

impl HookRunError {
    fn into_message(self, hook_name: &str) -> String {
        match self {
            Self::Cancelled => format!("Hook '{hook_name}' cancelled"),
            Self::TimedOut => {
                format!("Hook '{hook_name}' timed out after {HOOK_TIMEOUT_SECS} seconds")
            }
            Self::Message(message) => message,
        }
    }
}

fn configure_hook_command(cmd: &mut Command, command: &str, env_vars: &[(&str, &str)]) {
    cmd.arg("-c").arg(command);
    for (key, value) in env_vars {
        cmd.env(key, value);
    }
    // Hooks are side-effect only; never pipe I/O (unread stderr can deadlock the child).
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Unix: own process group so `kill` tears down `sh -c` children, not just the shell stub.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.as_std_mut().process_group(0);
    }
}

/// Send SIGKILL (via `Child::kill`), then `wait()` with a bounded reap timeout.
async fn terminate_hook_child(child: &mut Child) {
    let _ = child.kill().await;
    let reap = Duration::from_secs(HOOK_REAP_TIMEOUT_SECS);
    let _ = tokio::time::timeout(reap, child.wait()).await;
}

/// Run `sh -c` for a hook: `tokio::select!` on cancel, wall-clock timeout, or natural exit.
async fn run_hook_shell(
    hook_name: &str,
    command: &str,
    env_vars: &[(&str, &str)],
    cancel: &CancellationToken,
) -> Result<i32, HookRunError> {
    let mut cmd = Command::new("sh");
    configure_hook_command(&mut cmd, command, env_vars);

    let mut child = cmd.spawn().map_err(|e| {
        HookRunError::Message(format!("Failed to spawn hook '{hook_name}': {e}"))
    })?;

    let timeout = hook_run_timeout();

    tokio::select! {
        () = cancel.cancelled() => {
            terminate_hook_child(&mut child).await;
            Err(HookRunError::Cancelled)
        }
        () = tokio::time::sleep(timeout) => {
            terminate_hook_child(&mut child).await;
            Err(HookRunError::TimedOut)
        }
        result = child.wait() => {
            let status = result
                .map_err(|e| HookRunError::Message(format!("Hook wait error: {e}")))?;
            Ok(status.code().unwrap_or(1))
        }
    }
}

impl ShellHook {
    fn matches_tool(&self, tool_name: &str) -> bool {
        self.tool_pattern == "*" || self.tool_pattern == tool_name
    }

    async fn run_command(
        &self,
        env_vars: &[(&str, &str)],
        cancel: &CancellationToken,
    ) -> Result<i32, HookRunError> {
        run_hook_shell(&self.name, &self.command, env_vars, cancel).await
    }
}

#[async_trait::async_trait]
impl Hook for ShellHook {
    async fn pre_execute(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        cancel: &CancellationToken,
    ) -> Result<Option<String>, String> {
        if self.phase != HookPhase::Pre || !self.matches_tool(tool_name) {
            return Ok(None);
        }

        let params_str = params.to_string();
        let env_vars = vec![
            ("TOOL_NAME", tool_name),
            ("TOOL_PARAMS", params_str.as_str()),
        ];

        match self.run_command(&env_vars, cancel).await {
            Ok(0) => Ok(None),
            Ok(code) => Err(format!("Pre-hook '{}' exited with code {code}", self.name)),
            Err(err) => Err(err.into_message(&self.name)),
        }
    }

    async fn post_execute(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        output: &str,
        cancel: &CancellationToken,
    ) -> Result<String, String> {
        if self.phase != HookPhase::Post || !self.matches_tool(tool_name) {
            return Ok(output.to_string());
        }

        let params_str = params.to_string();
        let truncated_output: String = output.chars().take(1000).collect();
        let env_vars = vec![
            ("TOOL_NAME", tool_name),
            ("TOOL_PARAMS", params_str.as_str()),
            ("TOOL_OUTPUT", truncated_output.as_str()),
        ];

        match self.run_command(&env_vars, cancel).await {
            Ok(_) | Err(_) => Ok(output.to_string()),
        }
    }
}

pub fn shell_hooks_from_toml(contents: &str) -> Vec<ShellHook> {
    let Ok(doc) = contents.parse::<DocumentMut>() else {
        return vec![];
    };
    let mut entries = Vec::new();
    () = collect_flat_string_entries(doc.as_item(), "", &mut entries);
    parse_hooks_from_flat_config(&entries)
}

fn collect_flat_string_entries(item: &Item, prefix: &str, out: &mut Vec<(String, String)>) {
    match item {
        Item::Value(Value::String(s)) if !prefix.is_empty() => {
            out.push((String::from(prefix), s.value().clone()));
        }
        Item::Table(table) => {
            for (key, value) in table.iter() {
                let full_key = if prefix.is_empty() {
                    String::from(key)
                } else {
                    format!("{prefix}.{key}")
                };
                () = collect_flat_string_entries(value, &full_key, out);
            }
        }
        _ => {}
    }
}

/// Parse shell hook definitions from flat config keys (`hooks.pre.<tool>`, `hooks.post.<tool>`).
pub fn parse_hooks_from_flat_config(entries: &[(String, String)]) -> Vec<ShellHook> {
    let mut hooks = Vec::new();
    let mut keys: Vec<&String> = entries
        .iter()
        .filter(|(k, _)| k.starts_with("hooks."))
        .map(|(k, _)| k)
        .collect();
    () = keys.sort();

    for key in keys {
        let value = entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or_default();

        let rest = &key["hooks.".len()..];
        let (phase, tool_pattern) = if let Some(tool) = rest.strip_prefix("pre.") {
            (HookPhase::Pre, tool)
        } else if let Some(tool) = rest.strip_prefix("post.") {
            (HookPhase::Post, tool)
        } else {
            continue;
        };

        if tool_pattern.is_empty() || value.is_empty() {
            continue;
        }

        let phase_str = match phase {
            HookPhase::Pre => "pre",
            HookPhase::Post => "post",
        };

        () = hooks.push(ShellHook {
            name: format!("{phase_str}:{tool_pattern}"),
            phase,
            tool_pattern: String::from(tool_pattern),
            command: String::from(value),
        });
    }

    hooks
}

pub fn build_hook_registry(shell_hooks: &[ShellHook]) -> Arc<HookRegistry> {
    let mut registry = HookRegistry::new();
    for hook in shell_hooks {
        () = registry.register(Box::new(hook.clone()));
    }
    Arc::new(registry)
}

pub fn wrap_tools_with_hooks(
    tools: Vec<Box<dyn AgentTool>>,
    registry: &Arc<HookRegistry>,
) -> Vec<Box<dyn AgentTool>> {
    if registry.is_empty() {
        tools
    } else {
        tools
            .into_iter()
            .map(|tool| maybe_hook(tool, registry))
            .collect()
    }
}

struct HookedTool {
    inner: Box<dyn AgentTool>,
    hooks: Arc<HookRegistry>,
}

#[async_trait::async_trait]
impl AgentTool for HookedTool {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn label(&self) -> &str {
        self.inner.label()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.inner.parameters_schema()
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: yoagent::types::ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let cancel = ctx.cancel.clone();

        match self
            .hooks
            .run_pre_hooks(self.inner.name(), &params, &cancel)
            .await
        {
            Err(reason) if hook_error_is_cancelled(&reason) => return Err(ToolError::Cancelled),
            Err(reason) => {
                return Err(ToolError::Failed(format!("Blocked by hook: {reason}")));
            }
            Ok(Some(cached)) => {
                return Ok(ToolResult {
                    content: vec![Content::Text { text: cached }],
                    details: serde_json::Value::default(),
                });
            }
            Ok(None) => {}
        }

        let result = self.inner.execute(params.clone(), ctx).await?;

        let output_text: String = result
            .content
            .iter()
            .filter_map(|c| match c {
                Content::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        match self
            .hooks
            .run_post_hooks(self.inner.name(), &params, &output_text, &cancel)
            .await
        {
            Ok(_) => Ok(result),
            Err(reason) if hook_error_is_cancelled(&reason) => Err(ToolError::Cancelled),
            Err(reason) => Err(ToolError::Failed(format!("Post-hook error: {reason}"))),
        }
    }
}

fn hook_error_is_cancelled(reason: &str) -> bool {
    reason.contains("cancelled")
}

pub fn hooks_output_lines(hooks: &[ShellHook]) -> Vec<String> {
    if hooks.is_empty() {
        return vec![
            "  No hooks configured.".dimmed().to_string(),
            String::new(),
            String::from("  Add hooks to config.toml:"),
            String::new(),
            String::from("    # Pre-hook: runs before every bash tool call"),
            String::from("    hooks.pre.bash = \"echo 'About to run bash'\""),
            String::new(),
            String::from("    # Post-hook: runs after every tool call (wildcard)"),
            String::from("    hooks.post.\"*\" = \"echo 'Tool finished'\""),
            String::new(),
            String::from("  Pre-hooks that exit non-zero block the tool."),
            String::from("  Post-hooks always pass through the tool output."),
            "  All hooks have a 5-second timeout.".dimmed().to_string(),
        ];
    }

    let mut lines = vec![format!(
        "{}",
        format!("  Active hooks ({}):", hooks.len()).dimmed()
    )];
    () = lines.push(String::new());

    for hook in hooks {
        let phase = match hook.phase {
            HookPhase::Pre => "pre",
            HookPhase::Post => "post",
        };
        () = lines.push(format!(
            "    {}  ({}, pattern: {})",
            hook.name.bold(),
            phase,
            hook.tool_pattern
        ));
        () = lines.push(format!("      command: {}", hook.command));
    }

    lines
}

fn maybe_hook(tool: Box<dyn AgentTool>, hooks: &Arc<HookRegistry>) -> Box<dyn AgentTool> {
    if hooks.is_empty() {
        tool
    } else {
        Box::new(HookedTool {
            inner: tool,
            hooks: Arc::clone(hooks),
        })
    }
}
