use crate::providers::KNOWN_PROVIDER_IDS;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    disabled_providers: BTreeSet<String>,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        Self::load_from(&config_path())
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&config_path())
    }

    pub fn is_enabled(&self, provider: &str) -> bool {
        !self.disabled_providers.contains(provider)
    }

    pub fn set_enabled(&mut self, provider: &str, enabled: bool) {
        if enabled {
            self.disabled_providers.remove(provider);
        } else {
            self.disabled_providers.insert(provider.to_owned());
        }
    }

    pub fn enabled_provider_ids(&self) -> Vec<&'static str> {
        KNOWN_PROVIDER_IDS
            .iter()
            .copied()
            .filter(|provider| self.is_enabled(provider))
            .collect()
    }

    fn load_from(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content)
                .map_err(|error| format!("failed to parse {}: {error}", path.display())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("failed to read {}: {error}", path.display())),
        }
    }

    fn save_to(&self, path: &Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or_else(|| format!("invalid config path: {}", path.display()))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;

        let content = serde_json::to_string_pretty(self)
            .map_err(|error| format!("failed to serialize config: {error}"))?;
        fs::write(path, format!("{content}\n"))
            .map_err(|error| format!("failed to write {}: {error}", path.display()))
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("agent-limits")
        .join("config.json")
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_config_enables_all_providers() {
        let path = temp_path("missing");
        let config = Config::load_from(&path).unwrap();
        assert!(config.is_enabled("claude"));
        assert!(config.is_enabled("codex"));
        assert!(config.is_enabled("opencodego"));
    }

    #[test]
    fn disabled_provider_round_trips() {
        let path = temp_path("round-trip");
        let mut config = Config::default();
        config.set_enabled("opencodego", false);
        config.save_to(&path).unwrap();

        let loaded = Config::load_from(&path).unwrap();
        assert!(!loaded.is_enabled("opencodego"));
        assert!(loaded.is_enabled("claude"));

        let _ = fs::remove_file(&path);
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("agent-limits-{name}-{nonce}"))
            .join("config.json")
    }
}
