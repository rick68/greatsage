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
    std::{collections::hash_map::HashMap, fs, path::Path},
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

fn analyze_structure(path: &Path) -> Result<String, ToolCallError> {
    let mut file_count: i32 = 0;
    let mut dir_count: i32 = 0;
    let mut total_lines: usize = 0;
    let mut extensions: HashMap<String, usize> = HashMap::new();

    if path.is_file() {
        file_count = 1;
        if let Ok(content) = fs::read_to_string::<&Path>(path) {
            total_lines = content.lines().count();
        }
        if let Some(ext) = path.extension() {
            *extensions
                .entry(ext.to_string_lossy().to_string())
                .or_insert(0) += 1;
        }
    } else {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map::<DirEntry, fn(walkdir::Result<DirEntry>) -> Option<DirEntry>>(
                |e: walkdir::Result<DirEntry>| -> Option<DirEntry> { e.ok() },
            )
        {
            let entry_path: &Path = entry.path();
            if entry_path.is_file() {
                file_count += 1;
                if let Ok(content) = fs::read_to_string(entry_path) {
                    total_lines += content.lines().count();
                }
                if let Some(ext) = entry_path.extension() {
                    *extensions
                        .entry(ext.to_string_lossy().into_owned())
                        .or_insert(0) += 1;
                }
            } else if entry_path.is_dir() {
                dir_count += 1;
            }
        }
    }

    let mut ext_summary: String = String::new();
    for (ext, count) in extensions.iter() {
        () = ext_summary.push_str(&format!("\n  .{ext}: {count} files"));
    }

    Ok(format!(
        "Code Structure Analysis:\n\
        - Files: {file_count}\n\
        - Directories: {dir_count}\n\
        - Total lines: {total_lines}\n\
        - File types:{ext_summary}",
    ))
}

fn analyze_complexity(_path: &Path) -> Result<String, ToolCallError> {
    // Simplified complexity analysis
    Ok(String::from(
        "Complexity analysis: This is a placeholder. In a real implementation, \
        this would calculate cyclomatic complexity, function lengths, and other metrics.",
    ))
}

fn analyze_dependencies(_path: &Path) -> Result<String, ToolCallError> {
    // Simplified dependency analysis
    Ok(String::from(
        "Dependency analysis: This is a placeholder. In a real implementation, \
        this would parse import statements and analyze module dependencies.",
    ))
}

#[derive(Serialize, Deserialize, ToolInput, Debug)]
pub struct AnalyzeCodeArgs {
    #[input(description = "Path to the file or directory to analyze")]
    path: String,
    #[input(description = "Type of analysis: 'structure', 'complexity', 'dependencies'")]
    analysis_type: String,
}

#[tool(
    name = "AnalyzeCodeTool",
    description = "Analyze code structure, complexity, or dependencies",
    input = AnalyzeCodeArgs,
)]
pub struct AnalyzeCodeTool {}

#[async_trait]
impl ToolRuntime for AnalyzeCodeTool {
    async fn execute(&self, args: Value) -> Result<Value, ToolCallError> {
        let args: AnalyzeCodeArgs = serde_json::from_value(args)?;
        println!("🔬 Analyzing code: {} ({})", args.path, args.analysis_type);

        let path: &Path = Path::new(&args.path);
        if !path.exists() {
            return Err(ToolCallError::RuntimeError(
                anyhow::anyhow!(format!("Path {} does not exist", args.path))
                    .into_boxed_dyn_error(),
            ));
        }

        match args.analysis_type.as_str() {
            "structure" => Ok(Value::from(analyze_structure(path)?)),
            "complexity" => Ok(Value::from(analyze_complexity(path)?)),
            "dependencies" => Ok(Value::from(analyze_dependencies(path)?)),
            _ => Err(ToolCallError::RuntimeError(
                anyhow::anyhow!(
                    "Invalid analysis type. Choose 'structure', 'complexity', or 'dependencies'",
                )
                .into_boxed_dyn_error(),
            )),
        }
    }
}
