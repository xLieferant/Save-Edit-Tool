use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::autosave_path;
use crate::log;
use tauri::command;
use std::env;
use regex::Regex;
use std::fs;

#[command]
pub fn edit_money(amount: i64) -> Result<(), String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(r"info_money_account:\s*\d+").unwrap();
    let new = re.replace(&content, format!("info_money_account: {}", amount));
    fs::write(&path, new.as_bytes()).map_err(|e| e.to_string())?;
    log!("Geld geändert: {}", amount);
    Ok(())
}

#[command]
pub fn edit_xp(xp: i64) -> Result<(), String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(r"info_players_experience:\s*\d+").unwrap();
    let new = re.replace(&content, format!("info_players_experience: {}", xp));
    fs::write(&path, new.as_bytes()).map_err(|e| e.to_string())?;
    log!("XP geändert: {}", xp);
    Ok(())
}

#[command]
pub fn edit_level(level: i64) -> Result<(), String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(r"info_player_level:\s*\d+").unwrap();
    let new = re.replace(&content, format!("info_player_level: {}", level));
    fs::write(&path, new.as_bytes()).map_err(|e| e.to_string())?;
    log!("Level geändert: {}", level);
    Ok(())
}
