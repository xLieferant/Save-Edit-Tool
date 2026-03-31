use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::state::AppMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HubConfig {
    #[serde(default = "default_mode")]
    default_mode: AppMode,
}

fn default_mode() -> AppMode {
    AppMode::Utility
}

pub fn default_config_path() -> PathBuf {
    dirs::config_local_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("SimNexus")
        .join("hub.json")
}

pub fn load_mode() -> Result<AppMode, String> {
    let path = default_config_path();
    if !path.exists() {
        return Ok(AppMode::Utility);
    }

    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let config: HubConfig = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(config.default_mode)
}

pub fn save_mode(mode: AppMode) -> Result<(), String> {
    let path = default_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let raw = serde_json::to_string_pretty(&HubConfig { default_mode: mode })
        .map_err(|e| e.to_string())?;
    std::fs::write(path, format!("{raw}\n")).map_err(|e| e.to_string())
}
