use tauri::command;
use serde::Deserialize;
use std::fs;
use regex::Regex;

use crate::utils::paths::{
    autosave_path_current,
    base_config_path
};
use crate::utils::decrypt::decrypt_if_needed;
use crate::log;

#[derive(Deserialize)]
pub struct ApplyPayload {
    pub key: String,
    pub value: serde_json::Value,
}

#[command]
pub fn apply_setting(payload: ApplyPayload) -> Result<(), String> {
    let (path, regex, replacement) = match payload.key.as_str() {

        // ---------------- PROFILE / AUTOSAVE ----------------
        "money" => (
            autosave_path_current()?,
            r"info_money_account:\s*\d+",
            format!("info_money_account: {}", payload.value),
        ),

        "xp" => (
            autosave_path_current()?,
            r"info_players_experience:\s*\d+",
            format!("info_players_experience: {}", payload.value),
        ),

        // ---------------- BASE CONFIG ----------------
        "traffic" => (
            base_config_path()?,
            r"traffic:\s*\d+",
            format!("traffic: {}", payload.value),
        ),

        "developer" => (
            base_config_path()?,
            r"developer:\s*\d+",
            format!("developer: {}", payload.value),
        ),

        "console" => (
            base_config_path()?,
            r"console:\s*\d+",
            format!("console: {}", payload.value),
        ),

        _ => return Err(format!("Unknown setting key: {}", payload.key)),
    };

    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(regex).map_err(|e| e.to_string())?;

    let new_content = re.replace(&content, replacement).to_string();
    fs::write(&path, new_content).map_err(|e| e.to_string())?;

    log!("apply_setting OK → {}", payload.key);
    Ok(())
}
