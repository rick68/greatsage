use {std::path::Path, log::warn, serde::Deserialize};

#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
pub struct Config {
    pub provider: Option<crate::providers::Provider>,
    // future settings can be added here
}

#[allow(dead_code)]
impl Config {
    #[allow(dead_code)]
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            warn!("Config file not found: {}", path.display());
            return Self::default();
        }
        let content = std::fs::read_to_string(path).unwrap_or_else(|e| {
            warn!("Failed to read config file {}: {}", path.display(), e);
            String::new()
        });
        toml::from_str(&content).unwrap_or_else(|e| {
            warn!("Failed to parse config file {}: {}", path.display(), e);
            Self::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::fs};

    #[test]
    fn load_nonexistent_returns_default() {
        let mut path = std::env::temp_dir();
        path.push("nonexistent_config.toml");
        // Ensure file does not exist
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        let cfg = Config::load(&path);
        assert!(cfg.provider.is_none());
    }
}
