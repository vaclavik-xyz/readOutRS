use crate::config::AppConfiguration;
use std::path::PathBuf;

pub fn default_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("readout").join("config.json")
}

pub fn load(path: &std::path::Path) -> Result<AppConfiguration, ConfigStoreError> {
    if !path.exists() {
        return Ok(AppConfiguration::default());
    }
    let data =
        std::fs::read_to_string(path).map_err(|e| ConfigStoreError::ReadFailed(e.to_string()))?;
    serde_json::from_str(&data).map_err(|e| ConfigStoreError::ParseFailed(e.to_string()))
}

pub fn save(config: &AppConfiguration, path: &std::path::Path) -> Result<(), ConfigStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ConfigStoreError::WriteFailed(e.to_string()))?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| ConfigStoreError::SerializeFailed(e.to_string()))?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &json).map_err(|e| ConfigStoreError::WriteFailed(e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| ConfigStoreError::WriteFailed(e.to_string()))
}

#[derive(Debug)]
pub enum ConfigStoreError {
    ReadFailed(String),
    ParseFailed(String),
    WriteFailed(String),
    SerializeFailed(String),
}
