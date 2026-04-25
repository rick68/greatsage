use {
    bevy::ecs::resource::Resource,
    clap::{Subcommand, ValueEnum},
    serde::{Deserialize, Serialize},
    std::{
        env, fs,
        path::{Path, PathBuf},
    },
};

/// Runtime-only parameters parsed from CLI flags; never persisted to the config file.
#[derive(Clone, Debug, Default)]
pub struct RuntimeConfig {
    pub skills: Vec<PathBuf>,
    pub mcp_servers: Vec<String>,
    pub verbose: bool,
}

#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("invalid value for {key}: {reason}")]
    InvalidValue { key: String, reason: String },
    #[error("{0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

pub type ConfigResult<T> = Result<T, ConfigError>;

pub fn default_config_path() -> PathBuf {
    // Follow XDG: respect $XDG_CONFIG_HOME, fall back to ~/.config on all platforms.
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("greatsage").join("config.toml")
}

/// Thinking/reasoning intensity passed to `Agent::with_thinking`.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    #[default]
    Off,
    Minimal,
    Low,
    Medium,
    High,
}

/// Context management strategy.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ContextStrategy {
    /// Auto-compact conversation when approaching context limit
    #[default]
    Compaction,
    /// Write checkpoint file and exit with code 2 when approaching limit
    Checkpoint,
}

#[derive(Clone, Debug, Deserialize, Resource, Serialize)]
#[serde(default)]
pub struct LlmFileConfig {
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub context_window: u32,
    pub thinking_level: ThinkingLevel,
    pub temperature: Option<f32>,
}

impl Default for LlmFileConfig {
    fn default() -> Self {
        Self {
            model: String::from("claude-opus-4-7"),
            base_url: String::new(),
            max_tokens: 4096,
            context_window: 128_000,
            thinking_level: ThinkingLevel::Medium,
            temperature: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentFileConfig {
    pub max_retry_attempts: usize,
    pub context_strategy: ContextStrategy,
    pub max_turns: usize,
}

impl Default for AgentFileConfig {
    fn default() -> Self {
        Self {
            max_retry_attempts: 3,
            context_strategy: ContextStrategy::default(),
            max_turns: 50,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsFileConfig {
    pub max_output_chars: usize,
    pub truncation_head_lines: usize,
    pub truncation_tail_lines: usize,
}

impl Default for ToolsFileConfig {
    fn default() -> Self {
        Self {
            max_output_chars: 40_000,
            truncation_head_lines: 100,
            truncation_tail_lines: 50,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiFileConfig {
    pub frames_per_second: f32,
    pub cursor_blink_ms: u64,
}

impl Default for TuiFileConfig {
    fn default() -> Self {
        Self {
            frames_per_second: 30.0,
            cursor_blink_ms: 530,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpFileConfig {
    pub sse_transports: Vec<String>,
    pub stdio_transports: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PermissionsFileConfig {
    /// Empty string means "use current working directory"
    pub allowed_dir: String,
}

#[derive(Clone, Debug, Default, Deserialize, Resource, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub llm: LlmFileConfig,
    pub agent: AgentFileConfig,
    pub tools: ToolsFileConfig,
    pub tui: TuiFileConfig,
    pub mcp: McpFileConfig,
    pub permissions: PermissionsFileConfig,
    #[serde(skip)]
    pub runtime: RuntimeConfig,
}

impl AppConfig {
    /// Load config from `path`, creating it with defaults if absent.
    /// Missing fields in an existing file are filled with defaults and written back.
    pub fn load_or_create(path: &Path) -> Self {
        if path.exists() {
            let cfg = fs::read_to_string(path)
                .ok()
                .and_then(|s| toml::from_str::<AppConfig>(&s).ok())
                .unwrap_or_default();
            // Write back to fill in any fields added since the file was created.
            _ = cfg.save(path);
            cfg
        } else {
            let cfg = AppConfig::default();
            _ = cfg.save(path);
            cfg
        }
    }

    pub fn save(&self, path: &Path) -> ConfigResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_field(&self, key: &ConfigKey) -> String {
        match key {
            ConfigKey::LlmModel => self.llm.model.clone(),
            ConfigKey::LlmBaseUrl => self.llm.base_url.clone(),
            ConfigKey::LlmMaxTokens => self.llm.max_tokens.to_string(),
            ConfigKey::LlmContextWindow => self.llm.context_window.to_string(),
            ConfigKey::LlmThinkingLevel => format!("{:?}", self.llm.thinking_level).to_lowercase(),
            ConfigKey::AgentMaxRetryAttempts => self.agent.max_retry_attempts.to_string(),
            ConfigKey::AgentContextStrategy => {
                format!("{:?}", self.agent.context_strategy).to_lowercase()
            }
            ConfigKey::ToolsMaxOutputChars => self.tools.max_output_chars.to_string(),
            ConfigKey::ToolsTruncationHeadLines => self.tools.truncation_head_lines.to_string(),
            ConfigKey::ToolsTruncationTailLines => self.tools.truncation_tail_lines.to_string(),
            ConfigKey::TuiFramesPerSecond => self.tui.frames_per_second.to_string(),
            ConfigKey::TuiCursorBlinkMs => self.tui.cursor_blink_ms.to_string(),
            ConfigKey::PermissionsAllowedDir => self.permissions.allowed_dir.clone(),
            ConfigKey::McpSseTransports => self.mcp.sse_transports.join(", "),
            ConfigKey::McpStdioTransports => self.mcp.stdio_transports.join(", "),
        }
    }

    pub fn set_field(&mut self, key: &ConfigKey, value: &str) -> ConfigResult<()> {
        let key_name = || key.to_possible_value().unwrap().get_name().to_string();
        let parse_uint = |v: &str| -> ConfigResult<usize> {
            v.parse().map_err(|_| ConfigError::InvalidValue {
                key: key_name(),
                reason: format!("'{v}' is not a whole number"),
            })
        };
        match key {
            ConfigKey::LlmModel => self.llm.model = value.to_string(),
            ConfigKey::LlmBaseUrl => self.llm.base_url = value.to_string(),
            ConfigKey::LlmMaxTokens => {
                self.llm.max_tokens = if value.is_empty() {
                    0
                } else {
                    parse_uint(value)? as u32
                };
            }
            ConfigKey::LlmContextWindow => {
                self.llm.context_window = if value.is_empty() {
                    0
                } else {
                    parse_uint(value)? as u32
                };
            }
            ConfigKey::LlmThinkingLevel => {
                self.llm.thinking_level = match value {
                    "off" => ThinkingLevel::Off,
                    "minimal" => ThinkingLevel::Minimal,
                    "low" => ThinkingLevel::Low,
                    "medium" => ThinkingLevel::Medium,
                    "high" => ThinkingLevel::High,
                    _ => {
                        return Err(ConfigError::InvalidValue {
                            key: key_name(),
                            reason: format!(
                                "'{value}' is not valid; expected off/minimal/low/medium/high"
                            ),
                        });
                    }
                };
            }
            ConfigKey::AgentMaxRetryAttempts => {
                self.agent.max_retry_attempts = parse_uint(value)?;
            }
            ConfigKey::AgentContextStrategy => {
                self.agent.context_strategy = match value {
                    "compaction" => ContextStrategy::Compaction,
                    "checkpoint" => ContextStrategy::Checkpoint,
                    _ => {
                        return Err(ConfigError::InvalidValue {
                            key: key_name(),
                            reason: format!(
                                "'{value}' is not valid; expected 'compaction' or 'checkpoint'"
                            ),
                        });
                    }
                };
            }
            ConfigKey::ToolsMaxOutputChars => {
                self.tools.max_output_chars = parse_uint(value)?;
            }
            ConfigKey::ToolsTruncationHeadLines => {
                self.tools.truncation_head_lines = parse_uint(value)?;
            }
            ConfigKey::ToolsTruncationTailLines => {
                self.tools.truncation_tail_lines = parse_uint(value)?;
            }
            ConfigKey::TuiFramesPerSecond => {
                self.tui.frames_per_second =
                    value.parse().map_err(|_| ConfigError::InvalidValue {
                        key: key_name(),
                        reason: format!("'{value}' is not a number"),
                    })?;
            }
            ConfigKey::TuiCursorBlinkMs => {
                self.tui.cursor_blink_ms =
                    value.parse().map_err(|_| ConfigError::InvalidValue {
                        key: key_name(),
                        reason: format!("'{value}' is not a whole number"),
                    })?;
            }
            ConfigKey::PermissionsAllowedDir => {
                self.permissions.allowed_dir = value.to_string();
            }
            ConfigKey::McpSseTransports => {
                self.mcp.sse_transports = value
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect();
            }
            ConfigKey::McpStdioTransports => {
                self.mcp.stdio_transports = value
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect();
            }
        }
        Ok(())
    }
}

/// All settable config keys, with dot-notation names for tab-completion.
#[derive(Clone, Debug, ValueEnum)]
pub enum ConfigKey {
    #[value(name = "llm.model")]
    LlmModel,
    #[value(name = "llm.base_url")]
    LlmBaseUrl,
    #[value(name = "llm.max_tokens")]
    LlmMaxTokens,
    #[value(name = "llm.context_window")]
    LlmContextWindow,
    #[value(name = "llm.thinking_level")]
    LlmThinkingLevel,
    #[value(name = "agent.max_retry_attempts")]
    AgentMaxRetryAttempts,
    #[value(name = "agent.context_strategy")]
    AgentContextStrategy,
    #[value(name = "tools.max_output_chars")]
    ToolsMaxOutputChars,
    #[value(name = "tools.truncation_head_lines")]
    ToolsTruncationHeadLines,
    #[value(name = "tools.truncation_tail_lines")]
    ToolsTruncationTailLines,
    #[value(name = "tui.frames_per_second")]
    TuiFramesPerSecond,
    #[value(name = "tui.cursor_blink_ms")]
    TuiCursorBlinkMs,
    #[value(name = "permissions.allowed_dir")]
    PermissionsAllowedDir,
    #[value(name = "mcp.sse_transports")]
    McpSseTransports,
    #[value(name = "mcp.stdio_transports")]
    McpStdioTransports,
}

#[derive(Clone, Subcommand, Debug)]
pub enum ConfigSubcommand {
    /// Print all settings in TOML format
    List,
    /// Get the value of a single setting
    Get { key: ConfigKey },
    /// Set a setting and save to the config file
    Set { key: ConfigKey, value: String },
}

pub fn run_config_subcommand(
    cmd: &ConfigSubcommand,
    config_path: &Path,
    config: &mut AppConfig,
) -> ConfigResult<()> {
    match cmd {
        ConfigSubcommand::List => {
            println!("# {}", config_path.display());
            print!("{}", toml::to_string_pretty(config)?);
        }
        ConfigSubcommand::Get { key } => {
            println!("{}", config.get_field(key));
        }
        ConfigSubcommand::Set { key, value } => {
            () = config.set_field(key, value)?;
            () = config.save(config_path)?;
            println!("{} = {value}", key.to_possible_value().unwrap().get_name());
        }
    }
    Ok(())
}

/// Validate required values, accepting config file as fallback for non-secret fields.
/// API_KEY must always come from the environment.
pub fn validate_required(config: &AppConfig) -> ConfigResult<()> {
    let mut missing = Vec::new();
    if env::var("API_KEY")
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
    {
        () = missing.push("API_KEY (environment variable)");
    }
    // BASE_URL: env var takes priority, config file as fallback
    if env::var("BASE_URL")
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
        && config.llm.base_url.trim().is_empty()
    {
        () = missing.push("BASE_URL (environment variable or llm.base_url in config)");
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(ConfigError::Other(
            format!("Missing required configuration: {}", missing.join(", ")).into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use {super::*, tempfile::tempdir};

    #[test]
    fn creates_config_file_with_defaults_when_absent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = AppConfig::load_or_create(&path);
        assert!(path.exists());
        assert_eq!(cfg.llm.model, "claude-opus-4-7");
        assert_eq!(cfg.tools.max_output_chars, 40_000);
    }

    #[test]
    fn loads_partial_config_and_fills_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        () = fs::write(&path, "[llm]\nmodel = \"my-model\"\n").unwrap();
        let cfg = AppConfig::load_or_create(&path);
        assert_eq!(cfg.llm.model, "my-model");
        // default filled in for missing field
        assert_eq!(cfg.tools.max_output_chars, 40_000);
    }

    #[test]
    fn get_and_set_field_roundtrip() {
        let mut cfg = AppConfig::default();
        () = cfg.set_field(&ConfigKey::LlmModel, "test-model").unwrap();
        assert_eq!(cfg.get_field(&ConfigKey::LlmModel), "test-model");
    }

    #[test]
    fn set_invalid_number_returns_invalid_value_error() {
        let mut cfg = AppConfig::default();
        let err = cfg
            .set_field(&ConfigKey::ToolsMaxOutputChars, "not-a-number")
            .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidValue { .. }));
    }

    #[test]
    fn set_invalid_context_strategy_returns_invalid_value_error() {
        let mut cfg = AppConfig::default();
        let err = cfg
            .set_field(&ConfigKey::AgentContextStrategy, "unknown")
            .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidValue { .. }));
    }
}
