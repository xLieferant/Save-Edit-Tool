use regex::Regex;
use std::fs;
use std::path::Path;
use tauri::command;

use crate::utils::paths::autosave_path;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::extract::extract_value;

/// Interne Hilfsfunktion: ersetzt key: NUMBER (beibehaltung leading whitespace) mit neuem Wert.
/// Nutzt multiline Regex, ersetzt nur die erste passende Zeile.
fn replace_key_value_once(content: &str, key: &str, new_value: i64) -> String {
    // ^(\s*key\s*:\s*)-?\d+
    let pattern = format!(r"(?m)^(\s*{}\s*:\s*)-?\d+", regex::escape(key));
    let re = Regex::new(&pattern).unwrap();
    let replacement = format!("${{1}}{}", new_value);
    re.replace(content, replacement.as_str()).to_string()
}

fn write_back(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("Fehler beim Schreiben: {}", e))
}

#[command]
pub fn edit_money(profile_path: String, new_value: i64) -> Result<String, String> {
    let autosave = autosave_path(&profile_path);

    if !autosave.exists() {
        return Err(format!("Autosave nicht gefunden: {}", autosave.display()));
    }

    let content = decrypt_if_needed(&autosave)?;
    let prev = extract_value(&content, "money_account").unwrap_or(0);

    let updated = replace_key_value_once(&content, "money_account", new_value);
    write_back(&autosave, &updated)?;

    Ok(format!("Money geändert: {} -> {}", prev, new_value))
}

#[command]
pub fn edit_xp(profile_path: String, new_value: i64) -> Result<String, String> {
    let autosave = autosave_path(&profile_path);

    if !autosave.exists() {
        return Err(format!("Autosave nicht gefunden: {}", autosave.display()));
    }

    let content = decrypt_if_needed(&autosave)?;
    let prev = extract_value(&content, "experience_points").unwrap_or(0);

    let updated = replace_key_value_once(&content, "experience_points", new_value);
    write_back(&autosave, &updated)?;

    Ok(format!("XP geändert: {} -> {}", prev, new_value))
}

#[command]
pub fn edit_level(profile_path: String, new_value: i64) -> Result<String, String> {
    let autosave = autosave_path(&profile_path);

    if !autosave.exists() {
        return Err(format!("Autosave nicht gefunden: {}", autosave.display()));
    }

    let content = decrypt_if_needed(&autosave)?;
    let prev = extract_value(&content, "info_player_level").unwrap_or(0);

    let updated = replace_key_value_once(&content, "info_player_level", new_value);
    write_back(&autosave, &updated)?;

    Ok(format!("Level geändert: {} -> {}", prev, new_value))
}
