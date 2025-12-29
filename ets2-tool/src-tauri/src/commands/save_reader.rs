use crate::log; // This import is now used
use crate::models::save_game_data::SaveGameData;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::autosave_path;
use crate::utils::paths::ets2_base_config_path;
use crate::utils::current_profile::{get_current_profile, require_current_profile };
use regex::Regex;
use std::fs;
use tauri::command;

#[command]
pub fn read_money() -> Result<i64, String> {
    let profile = require_current_profile()?;

    log!("Lese Geld aus Profil: {}", profile); // Used here
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(r"info_money_account:\s*(\d+)").unwrap();
    Ok(re
        .captures(&content)
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0))
}

#[command]
pub fn read_xp() -> Result<i64, String> {
    let profile = require_current_profile()?;

    log!("Lese XP aus Profil: {}", profile); // Used here
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(r"info_players_experience:\s*(\d+)").unwrap();
    Ok(re
        .captures(&content)
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0))
}

#[command]
pub fn read_all_save_data() -> Result<SaveGameData, String> {
    let profile = require_current_profile()?;

    log!("Lese alle Speicherdaten aus Profil: {}", profile); // Used here
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;

    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = SaveGameData {
        money: re(r"info_money_account:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        xp: re(r"info_players_experience:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        recruitments: re(r"info_unlocked_recruitments:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        dealers: re(r"info_unlocked_dealers:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        visited_cities: re(r"info_visited_cities:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
    };
    log!(
        "Gefundene Daten: Geld: {:?}, XP: {:?}, Recruitments: {:?}, dealers: {:?}, visited_cities: {:?}",
        data.money,
        data.xp,
        data.recruitments,
        data.dealers,
        data.visited_cities,
    );
    Ok(data)
}

#[command]
pub fn read_traffic_value() -> Result<i64, String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_traffic\s+"(\d+)""#).map_err(|e| e.to_string())?;

    let caps = re
        .captures(&content)
        .and_then(|c| c.get(1))
        .ok_or("g_traffic nicht gefunden".to_string())?;

    let value = caps
        .as_str()
        .parse::<i64>()
        .map_err(|_| "Traffic-Wert ungültig".to_string())?;

    Ok(value.clamp(0, 10))
}
