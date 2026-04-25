use {
    crate::agents::PermissionConfig,
    std::path::PathBuf,
    yoagent::{
        tools::default_tools,
        types::{AgentTool, Content, ToolContext, ToolError, ToolResult},
    },
};

const MAX_TOOL_OUTPUT_CHARS: usize = 40_000;
const TRUNCATION_HEAD_LINES: usize = 100;
const TRUNCATION_TAIL_LINES: usize = 50;

fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&p) = chars.peek() {
                    if p.is_ascii_digit() || p == ';' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Some(&f) = chars.peek()
                    && f.is_ascii_alphabetic()
                {
                    chars.next();
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn truncate_tool_output(output: &str, max_chars: usize) -> String {
    let stripped = strip_ansi_codes(output);
    if stripped.len() <= max_chars {
        return stripped;
    }
    let lines: Vec<&str> = stripped.lines().collect();
    let total_lines = lines.len();
    if total_lines <= TRUNCATION_HEAD_LINES + TRUNCATION_TAIL_LINES {
        return stripped;
    }
    let head = &lines[..TRUNCATION_HEAD_LINES];
    let tail = &lines[total_lines - TRUNCATION_TAIL_LINES..];
    let omitted = total_lines - TRUNCATION_HEAD_LINES - TRUNCATION_TAIL_LINES;
    let line_word = if omitted == 1 { "line" } else { "lines" };
    let mut result = String::with_capacity(max_chars);
    for line in head {
        result.push_str(line);
        result.push('\n');
    }
    () = result.push_str(&format!("\n[... truncated {omitted} {line_word} ...]\n\n"));
    for (i, line) in tail.iter().enumerate() {
        result.push_str(line);
        if i < tail.len() - 1 {
            result.push('\n');
        }
    }
    result
}

struct TruncatingTool {
    inner: Box<dyn AgentTool>,
    max_chars: usize,
}

#[async_trait::async_trait]
impl AgentTool for TruncatingTool {
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
        ctx: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let result = self.inner.execute(params, ctx).await?;
        let content = result
            .content
            .into_iter()
            .map(|c| match c {
                Content::Text { text } => Content::Text {
                    text: truncate_tool_output(&text, self.max_chars),
                },
                other => other,
            })
            .collect();
        Ok(ToolResult {
            content,
            details: result.details,
        })
    }
}

struct PermissionGuardTool {
    inner: Box<dyn AgentTool>,
    allowed_dir: PathBuf,
}

#[async_trait::async_trait]
impl AgentTool for PermissionGuardTool {
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
        ctx: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let perm = PermissionConfig {
            allowed_dir: self.allowed_dir.clone(),
        };
        match self.inner.name() {
            "bash" => {
                // Warn-only for bash: complex shell commands are hard to fully validate,
                // and blocking them breaks legitimate use cases like network access.
                let cmd = params.get("command").and_then(|v| v.as_str()).unwrap_or("");
                if let Err(e) = perm.validate_command(cmd) {
                    eprintln!("[permission] bash warning: {e}");
                }
            }
            "read_file" | "write_file" | "edit_file" | "list_files" | "search" => {
                let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
                if let Err(e) = perm.validate_path(path) {
                    return Err(ToolError::Failed(format!(
                        "{e} (allowed directory: {})",
                        self.allowed_dir.display()
                    )));
                }
            }
            _ => {}
        }
        self.inner.execute(params, ctx).await
    }
}

pub fn build_tools(allowed_dir: PathBuf) -> Vec<Box<dyn AgentTool>> {
    default_tools()
        .into_iter()
        .map(|tool| -> Box<dyn AgentTool> {
            Box::new(TruncatingTool {
                inner: Box::new(PermissionGuardTool {
                    inner: tool,
                    allowed_dir: allowed_dir.clone(),
                }),
                max_chars: MAX_TOOL_OUTPUT_CHARS,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn strips_ansi_codes() {
        let input = "\x1b[31mred\x1b[0m normal";
        assert_eq!(strip_ansi_codes(input), "red normal");
    }

    #[test]
    fn no_truncation_under_limit() {
        let short = "line1\nline2\nline3";
        assert_eq!(truncate_tool_output(short, 40_000), short);
    }

    #[test]
    fn truncates_head_and_tail() {
        let lines: Vec<String> = (0..200).map(|i| format!("line {i}")).collect();
        let input = lines.join("\n");
        let output = truncate_tool_output(&input, 0);
        assert!(output.contains("line 0"));
        assert!(output.contains("line 99"));
        assert!(output.contains("[... truncated 50 lines ...]"));
        assert!(output.contains("line 199"));
        assert!(!output.contains("line 100\n"));
    }

    #[test]
    fn no_truncation_when_few_lines() {
        let lines: Vec<String> = (0..130).map(|i| format!("line {i}")).collect();
        let input = lines.join("\n");
        // 130 lines ≤ 150 threshold, so no head/tail split even with max_chars=0
        let output = truncate_tool_output(&input, 0);
        assert!(!output.contains("truncated"));
    }
}
