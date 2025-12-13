use tauri::command;
use regex::Regex;
use std::fs;
use std::env;
use std::path::PathBuf;

use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::{autosave_path, config_path, game_sii_path};
use crate::log;

fn choose_path(file_type: &str, profile: &str) -> Result<PathBuf, String> {
    match file_type {
        "save" => Ok(autosave_path(profile)),
        "config" => Ok(config_path(profile)),
        "game" => Ok(game_sii_path(profile)),
        _ => Err(format!("Unknown file_type: {}", file_type)),
    }
}

fn format_value(raw: &str) -> String {
    let trimmed = raw.trim();

    if Regex::new(r"^-?\d+(\.\d+)?$").unwrap().is_match(trimmed) {
        return trimmed.to_string();
    }

    if trimmed == "0" || trimmed == "1" {
        return trimmed.to_string();
    }

    if trimmed.contains(' ') {
        return format!("\"{}\"", trimmed);
    }

    trimmed.to_string()
}

#[command]
pub fn apply_setting(
    key: String,
    value: String,
    file_type: String
) -> Result<(), String> {
    let profile = env::var("CURRENT_PROFILE")
        .map_err(|_| "No profile loaded".to_string())?;

    let path = choose_path(&file_type, &profile)?;
    let content = decrypt_if_needed(&path)?;
    let formatted = format_value(&value);

    let pattern = format!(r"(?m)^\s*{}\s*:\s*.*$", regex::escape(&key));
    let re = Regex::new(&pattern).map_err(|e| e.to_string())?;

    let replacement = format!("{}: {}", key, formatted);

    let new_content = if re.is_match(&content) {
        re.replace_all(&content, replacement.as_str()).into_owned()
    } else {
        format!("{}\n{}", content, replacement)
    };

    fs::write(&path, new_content).map_err(|e| e.to_string())?;

    log!("Applied setting {} = {}", key, value);
    Ok(())
}
