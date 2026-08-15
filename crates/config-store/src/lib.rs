use std::path::{Path, PathBuf};

use router_model::config::{RouterConfig, SCHEMA_VERSION};

#[derive(Debug, thiserror::Error)]
pub enum ConfigStoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(u32),
    #[error("config file has no path")]
    MissingPath,
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<RouterConfig, ConfigStoreError> {
        let bytes = std::fs::read(&self.path)?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        let schema = value
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(SCHEMA_VERSION as u64);

        if schema > SCHEMA_VERSION as u64 {
            return Err(ConfigStoreError::UnsupportedSchema(schema as u32));
        }

        let config: RouterConfig = serde_json::from_value(value)?;
        Ok(config)
    }

    pub fn save(&self, config: &RouterConfig) -> Result<(), ConfigStoreError> {
        let parent = self.path.parent().ok_or(ConfigStoreError::MissingPath)?;
        std::fs::create_dir_all(parent)?;
        let json = serde_json::to_vec_pretty(config)?;

        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn load_or_default(&self) -> RouterConfig {
        self.load().unwrap_or_else(|_| RouterConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use router_model::config::{RouterConfig, SCHEMA_VERSION};

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join("link-helm-config-store-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = ConfigStore::new(dir.join("config.json"));

        let config = RouterConfig {
            schema_version: SCHEMA_VERSION,
            rules: vec![],
        };
        store.save(&config).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded, config);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_newer_schema_without_overwriting() {
        let dir = std::env::temp_dir().join("link-helm-config-store-newer");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, r#"{"schema_version": 999, "rules": []}"#).unwrap();

        let store = ConfigStore::new(&path);
        assert!(matches!(
            store.load(),
            Err(ConfigStoreError::UnsupportedSchema(999))
        ));
        // original file must remain untouched
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"schema_version": 999, "rules": []}"#
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
