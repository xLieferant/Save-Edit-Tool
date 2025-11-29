use std::fs;
use std::path::Path;

use crate::utils::{decrypt::decrypt_if_needed, paths::autosave_path};

use tauri::command;

fn replace_value(content: &str, key: &str, new_value: i64) -> String {
    content
        .lines()
        .map(|line| {
            if line.trim().starts_with(key) {
                format!("{}: {}", key, new_value)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_back(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content)
        .map_err(|e| format!("Fehler beim Schreiben: {}", e))
}

#[command]
pub fn edit_money(profile_path: String, new_value: i64) -> Result<String, String> {
    let autosave = autosave_path(&profile_path);
    let txt = decrypt_if_needed(&autosave)?;

    let updated = replace_value(&txt, "money_account", new_value);
    write_back(&autosave, &updated)?;

    Ok("Geld erfolgreich aktualisiert".into())
}

#[command]
pub fn edit_xp(profile_path: String, new_value: i64) -> Result<String, String> {
    let autosave = autosave_path(&profile_path);
    let txt = decrypt_if_needed(&autosave)?;

    let updated = replace_value(&txt, "experience_points", new_value);
    write_back(&autosave, &updated)?;

    Ok("XP erfolgreich aktualisiert".into())
}

#[command]
pub fn edit_level(profile_path: String, new_value: i64) -> Result<String, String> {
    let autosave = autosave_path(&profile_path);
    let txt = decrypt_if_needed(&autosave)?;

    let updated = replace_value(&txt, "user_xp", new_value);
    write_back(&autosave, &updated)?;

    Ok("Level erfolgreich aktualisiert".into())
}
