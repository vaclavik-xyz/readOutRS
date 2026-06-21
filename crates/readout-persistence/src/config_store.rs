use crate::config::AppConfiguration;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub struct StartupConfiguration {
    pub config: AppConfiguration,
    pub first_run: bool,
}

pub fn default_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("readout").join("config.json")
}

pub fn load(path: &Path) -> Result<AppConfiguration, ConfigStoreError> {
    if !path.exists() {
        return Ok(AppConfiguration::default());
    }
    let data =
        std::fs::read_to_string(path).map_err(|e| ConfigStoreError::ReadFailed(e.to_string()))?;
    serde_json::from_str(&data).map_err(|e| ConfigStoreError::ParseFailed(e.to_string()))
}

pub fn load_for_startup(
    path: &Path,
    explicit_path: bool,
) -> Result<StartupConfiguration, ConfigStoreError> {
    let first_run = !path.exists();
    if explicit_path && first_run {
        return Err(ConfigStoreError::ReadFailed(format!(
            "config file does not exist: {}",
            path.display()
        )));
    }

    Ok(StartupConfiguration {
        config: load(path)?,
        first_run,
    })
}

pub fn save(config: &AppConfiguration, path: &Path) -> Result<(), ConfigStoreError> {
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
