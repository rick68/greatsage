use {
    autoagents::{
        async_trait,
        core::tool::{ToolCallError, ToolInputT, ToolRuntime, ToolT},
    },
    autoagents_derive::{ToolInput, tool},
    serde::{Deserialize, Serialize},
    serde_json::Value,
    std::{collections::hash_map::HashMap, fs, path::Path},
    walkdir::{DirEntry, WalkDir},
};

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
