use {
    autoagents::{
        async_trait,
        core::tool::{ToolCallError, ToolInputT, ToolRuntime, ToolT},
    },
    autoagents_derive::{ToolInput, tool},
    glob::{Pattern, PatternError},
    regex::Regex,
    serde::{Deserialize, Serialize},
    serde_json::Value,
    std::{fs, path::Path},
    walkdir::{DirEntry, WalkDir},
};

#[derive(Debug, Deserialize, Serialize, ToolInput)]
pub struct GrepArgs {
    #[input(description = "Regular expression pattern to search for")]
    pattern: String,
    #[input(description = "File glob pattern to search in (e.g., '*.rs')")]
    file_pattern: String,
    #[input(description = "Base directory to search in")]
    base_dir: String,
}

#[tool(
    name = "GrepTool",
    description = "Search for content in files using regex patterns",
    input = GrepArgs,
)]
pub struct GrepTool {}

#[async_trait]
impl ToolRuntime for GrepTool {
    async fn execute(&self, args: Value) -> Result<Value, ToolCallError> {
        let args: GrepArgs = serde_json::from_value::<GrepArgs>(args)?;
        println!("🔎 Grepping for: {} in {}", args.pattern, args.file_pattern);

        let regex: Regex = Regex::new(&args.pattern)
            .map_err::<ToolCallError, fn(regex::Error) -> ToolCallError>(
                |e: regex::Error| -> ToolCallError {
                    ToolCallError::RuntimeError(
                        anyhow::anyhow!(format!("Invalid regex: {e}")).into_boxed_dyn_error(),
                    )
                },
            )?;

        let base_path: &Path = Path::new(&args.base_dir);
        if !base_path.exists() {
            return Err(ToolCallError::RuntimeError(
                anyhow::anyhow!("Directory {} does not exist", args.base_dir)
                    .into_boxed_dyn_error(),
            ));
        }

        let file_pattern: Pattern =
            Pattern::new(&args.file_pattern)
                .map_err::<ToolCallError, fn(PatternError) -> ToolCallError>(
                    |e: PatternError| -> ToolCallError {
                        ToolCallError::RuntimeError(
                            anyhow::anyhow!("Invalid file pattern: {e}").into_boxed_dyn_error(),
                        )
                    },
                )?;

        let mut results: Vec<String> = Vec::new();
        let max_results: usize = 50;

        for entry in WalkDir::new::<&String>(&args.base_dir)
            .follow_links(true)
            .into_iter()
            .filter_map::<DirEntry, fn(walkdir::Result<DirEntry>) -> Option<DirEntry>>(
                |e: walkdir::Result<DirEntry>| -> Option<DirEntry> { e.ok() },
            )
        {
            if results.len() >= max_results {
                break;
            }

            let path: &Path = entry.path();
            if path.is_file() {
                let relative_path: &Path =
                    path.strip_prefix::<&String>(&args.base_dir).unwrap_or(path);
                if file_pattern.matches_path(relative_path)
                    && let Ok(content) = fs::read_to_string::<&Path>(path)
                {
                    for (line_num, line) in content.lines().enumerate() {
                        if regex.is_match(line) {
                            () = results.push(format!(
                                "{}:{}: {}",
                                relative_path.display(),
                                line_num + 1,
                                line.trim()
                            ));
                            if results.len() >= max_results {
                                break;
                            }
                        }
                    }
                }
            }
        }

        if results.is_empty() {
            Ok(Value::from("No matches found."))
        } else {
            Ok(Value::from(format!(
                "Found {} matches (showing up to {max_results}):\n{}",
                results.len(),
                results.join("\n")
            )))
        }
    }
}
