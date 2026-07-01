//! Scaffold `GREATSAGE.md` from a cwd project scan (`/init`).

use std::{fmt, fs, path::Path};

pub const GREATSAGE_MD: &str = "GREATSAGE.md";
pub const YOYO_MD: &str = "YOYO.md";
pub const CLAUDE_MD: &str = "CLAUDE.md";

/// Detected project type based on marker files in the working directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Java,
    Ruby,
    Cpp,
    Make,
    Unknown,
}

impl fmt::Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rust => write!(f, "Rust (Cargo)"),
            Self::Node => write!(f, "Node.js (npm)"),
            Self::Python => write!(f, "Python"),
            Self::Go => write!(f, "Go"),
            Self::Java => write!(f, "Java"),
            Self::Ruby => write!(f, "Ruby"),
            Self::Cpp => write!(f, "C/C++ (CMake)"),
            Self::Make => write!(f, "Makefile"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitOutcome {
    RefusedExists,
    RefusedCompatRename(&'static str),
    Created {
        line_count: usize,
        project_type: ProjectType,
    },
    IoError(String),
}

/// AI tool instruction files to mention in generated GREATSAGE.md (excluding GREATSAGE.md itself).
const AI_CONFIG_FILES: &[(&str, &str)] = &[
    (YOYO_MD, "yoyo-evolve"),
    ("CLAUDE.md", "Claude Code"),
    ("AGENTS.md", "Gemini / generic agents"),
    (".cursorrules", "Cursor"),
    (".github/copilot-instructions.md", "GitHub Copilot"),
];

pub fn detect_project_type(dir: &Path) -> ProjectType {
    if dir.join("Cargo.toml").exists() {
        ProjectType::Rust
    } else if dir.join("package.json").exists() {
        ProjectType::Node
    } else if dir.join("pom.xml").exists()
        || dir.join("build.gradle").exists()
        || dir.join("build.gradle.kts").exists()
    {
        ProjectType::Java
    } else if dir.join("Gemfile").exists() {
        ProjectType::Ruby
    } else if dir.join("pyproject.toml").exists()
        || dir.join("setup.py").exists()
        || dir.join("setup.cfg").exists()
    {
        ProjectType::Python
    } else if dir.join("go.mod").exists() {
        ProjectType::Go
    } else if dir.join("CMakeLists.txt").exists() {
        ProjectType::Cpp
    } else if dir.join("Makefile").exists() || dir.join("makefile").exists() {
        ProjectType::Make
    } else {
        ProjectType::Unknown
    }
}

pub fn scan_important_files(dir: &Path) -> Vec<String> {
    const CANDIDATES: &[&str] = &[
        "README.md",
        "README",
        "readme.md",
        "LICENSE",
        "LICENSE.md",
        "CHANGELOG.md",
        "CONTRIBUTING.md",
        ".gitignore",
        ".editorconfig",
        // Rust
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        // Node
        "package.json",
        "package-lock.json",
        "tsconfig.json",
        ".eslintrc.json",
        ".eslintrc.js",
        ".prettierrc",
        // Python
        "pyproject.toml",
        "setup.py",
        "setup.cfg",
        "requirements.txt",
        "Pipfile",
        "tox.ini",
        // Go
        "go.mod",
        "go.sum",
        // Java
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        // Ruby
        "Gemfile",
        "Gemfile.lock",
        "Rakefile",
        ".rubocop.yml",
        // C/C++
        "CMakeLists.txt",
        // Build/CI
        "Makefile",
        "Dockerfile",
        "docker-compose.yml",
        "docker-compose.yaml",
        ".dockerignore",
        // CI configs
        ".github/workflows",
        ".gitlab-ci.yml",
        ".circleci/config.yml",
        ".travis.yml",
        "Jenkinsfile",
    ];
    CANDIDATES
        .iter()
        .filter(|f| dir.join(f).exists())
        .map(|f| (*f).to_string())
        .collect()
}

pub fn scan_important_dirs(dir: &Path) -> Vec<String> {
    const CANDIDATES: &[&str] = &[
        "src",
        "lib",
        "tests",
        "test",
        "docs",
        "doc",
        "examples",
        "benches",
        "scripts",
        ".github",
        ".vscode",
        "config",
        "public",
        "static",
        "assets",
        "migrations",
    ];
    CANDIDATES
        .iter()
        .filter(|d| dir.join(d).is_dir())
        .map(|d| (*d).to_string())
        .collect()
}

pub fn build_commands_for_project(project_type: ProjectType) -> Vec<(&'static str, &'static str)> {
    match project_type {
        ProjectType::Rust => vec![
            ("Build", "cargo build"),
            ("Test", "cargo test"),
            ("Lint", "cargo clippy --all-targets -- -D warnings"),
            ("Format check", "cargo fmt -- --check"),
            ("Format", "cargo fmt"),
        ],
        ProjectType::Node => vec![
            ("Install", "npm install"),
            ("Test", "npm test"),
            ("Lint", "npx eslint ."),
        ],
        ProjectType::Python => vec![
            ("Test", "python -m pytest"),
            ("Lint", "ruff check ."),
            ("Type check", "python -m mypy ."),
        ],
        ProjectType::Go => vec![
            ("Build", "go build ./..."),
            ("Test", "go test ./..."),
            ("Vet", "go vet ./..."),
        ],
        ProjectType::Java => vec![("Build", "mvn compile"), ("Test", "mvn test")],
        ProjectType::Ruby => vec![
            ("Test", "bundle exec rake test"),
            ("Lint", "bundle exec rubocop"),
        ],
        ProjectType::Cpp => vec![
            ("Build", "cmake --build build"),
            ("Test", "ctest --test-dir build"),
        ],
        ProjectType::Make => vec![("Build", "make"), ("Test", "make test")],
        ProjectType::Unknown => vec![],
    }
}

fn extract_name_from_cargo_toml(dir: &Path) -> Option<String> {
    let content = fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name") {
            let rest = rest.trim().strip_prefix('=')?.trim();
            let val = rest.trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn extract_name_from_package_json(dir: &Path) -> Option<String> {
    let content = fs::read_to_string(dir.join("package.json")).ok()?;
    for line in content.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if let Some(rest) = trimmed.strip_prefix("\"name\"") {
            let rest = rest.trim().strip_prefix(':')?.trim().trim_matches('"');
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

fn extract_project_name_from_readme(dir: &Path) -> Option<String> {
    for name in ["README.md", "readme.md", "README"] {
        if let Ok(content) = fs::read_to_string(dir.join(name)) {
            for line in content.lines() {
                if let Some(title) = line.trim().strip_prefix("# ") {
                    let title = title.trim();
                    if !title.is_empty() {
                        return Some(title.to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn detect_project_name(dir: &Path) -> String {
    if let Some(name) = extract_name_from_cargo_toml(dir) {
        return name;
    }
    if let Some(name) = extract_name_from_package_json(dir) {
        return name;
    }
    if let Some(name) = extract_project_name_from_readme(dir) {
        return name;
    }
    dir.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "my-project".to_string())
}

pub fn detect_ai_config_files(dir: &Path) -> Vec<(&'static str, &'static str)> {
    AI_CONFIG_FILES
        .iter()
        .filter(|(path, _)| dir.join(path).exists())
        .copied()
        .collect()
}

pub fn generate_init_content(dir: &Path) -> String {
    let project_type = detect_project_type(dir);
    let project_name = detect_project_name(dir);
    let important_files = scan_important_files(dir);
    let important_dirs = scan_important_dirs(dir);
    let build_commands = build_commands_for_project(project_type);

    let mut content = String::new();
    () = content.push_str("# Project Context\n\n");
    () = content
        .push_str("<!-- GREATSAGE.md — generated by `greatsage /init`. Edit to customize. -->\n");
    () = content
        .push_str("<!-- Also works as CLAUDE.md for compatibility with other tools. -->\n\n");

    () = content.push_str("## About This Project\n\n");
    () = content.push_str(&format!("**{project_name}**"));
    if project_type != ProjectType::Unknown {
        () = content.push_str(&format!(" — {project_type} project"));
    }
    () = content.push_str("\n\n");
    () = content.push_str("<!-- Add a description of what this project does. -->\n\n");

    () = content.push_str("## Build & Test\n\n");
    if build_commands.is_empty() {
        () = content.push_str("<!-- Add build, test, and run commands for this project. -->\n\n");
    } else {
        () = content.push_str("```bash\n");
        for (label, cmd) in &build_commands {
            () = content.push_str(&format!("{cmd:<50} # {label}\n"));
        }
        () = content.push_str("```\n\n");
    }

    () = content.push_str("## Coding Conventions\n\n");
    () = content.push_str(
        "<!-- List any coding standards, naming conventions, or patterns to follow. -->\n\n",
    );

    content.push_str("## Important Files\n\n");
    if important_files.is_empty() && important_dirs.is_empty() {
        () = content
            .push_str("<!-- List key files and directories the agent should know about. -->\n");
    } else {
        if !important_dirs.is_empty() {
            () = content.push_str("Key directories:\n");
            for d in &important_dirs {
                () = content.push_str(&format!("- `{d}/`\n"));
            }
            () = content.push('\n');
        }
        if !important_files.is_empty() {
            () = content.push_str("Key files:\n");
            for f in &important_files {
                () = content.push_str(&format!("- `{f}`\n"));
            }
            () = content.push('\n');
        }
    }

    let ai_configs = detect_ai_config_files(dir);
    if !ai_configs.is_empty() {
        () = content.push_str("\n## Other AI Tool Configs\n\n");
        () = content.push_str("This project also has instruction files for other AI tools:\n");
        for (path, label) in &ai_configs {
            content.push_str(&format!("- `{path}` ({label})\n"));
        }
        () = content
            .push_str("\ngreatsage reads these automatically for additional project context.\n");
    }

    content
}

fn pluralize_lines(count: usize) -> &'static str {
    if count == 1 { "line" } else { "lines" }
}

pub fn init_greatsage_md(cwd: &Path) -> InitOutcome {
    let target = cwd.join(GREATSAGE_MD);
    if target.exists() {
        return InitOutcome::RefusedExists;
    }
    if cwd.join(YOYO_MD).exists() {
        return InitOutcome::RefusedCompatRename(YOYO_MD);
    }
    if cwd.join(CLAUDE_MD).exists() {
        return InitOutcome::RefusedCompatRename(CLAUDE_MD);
    }

    let project_type = detect_project_type(cwd);
    let content = generate_init_content(cwd);
    let line_count = content.lines().count();

    match fs::write(&target, &content) {
        Ok(()) => InitOutcome::Created {
            line_count,
            project_type,
        },
        Err(e) => InitOutcome::IoError(e.to_string()),
    }
}

pub fn init_repl_output_lines(outcome: &InitOutcome, ai_config_names: &[&str]) -> Vec<String> {
    match outcome {
        InitOutcome::RefusedExists => {
            vec![format!("{GREATSAGE_MD} already exists — not overwriting.")]
        }
        InitOutcome::RefusedCompatRename(source) => vec![
            format!("{source} already exists — greatsage reads it as a compatibility alias."),
            format!("Rename it to {GREATSAGE_MD} when you're ready: mv {source} {GREATSAGE_MD}"),
        ],
        InitOutcome::IoError(msg) => vec![format!("error creating {GREATSAGE_MD}: {msg}")],
        InitOutcome::Created {
            line_count,
            project_type,
        } => {
            let mut lines = vec![
                String::from("Scanning project..."),
                format!("Detected: {project_type}"),
            ];
            if !ai_config_names.is_empty() {
                lines.push(format!(
                    "Found existing AI configs: {} — greatsage reads these automatically",
                    ai_config_names.join(", ")
                ));
            }
            () = lines.push(format!(
                "✓ Created {GREATSAGE_MD} ({line_count} {}) — edit it to add project context.",
                pluralize_lines(*line_count)
            ));
            () = lines.push(String::from(
                "Tip: Use /remember to save project-specific notes that persist across sessions.",
            ));
            lines
        }
    }
}
