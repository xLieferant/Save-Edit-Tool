use crate::log;
use crate::models::save_game_data::SaveGameData;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::{autosave_path, ets2_base_config_path, game_sii_from_save};
use crate::utils::current_profile::{require_current_profile};
use regex::Regex;
use std::fs;
use std::path::Path;
use tauri::command;
use tauri::State;
use crate::state::AppProfileState;

fn get_active_save_path(profile_state: State<'_, AppProfileState>) -> Result<std::path::PathBuf, String> {
    let save_opt = profile_state.current_save.lock().unwrap().clone();
    if let Some(save) = save_opt {
        return Ok(game_sii_from_save(Path::new(&save)));
    }
    // Fallback: Autosave, falls kein Save explizit geladen wurde
    let profile = require_current_profile(profile_state)?;
    Ok(autosave_path(&profile))
}

#[command]
pub fn read_money(
    profile_state: State<'_, AppProfileState>,
) -> Result<i64, String> {
    let path = get_active_save_path(profile_state)?;
    log!("Lese Geld aus: {:?}", path);
    
    let content = decrypt_if_needed(&path)?;

    // 1. Versuch: Echtes Geld (money_account)
    // (?m)^\s* verhindert, dass wir "info_money_account" matchen
    let re_main = Regex::new(r"(?m)^\s*money_account:\s*(\d+)").unwrap();
    if let Some(cap) = re_main.captures(&content) {
        if let Ok(val) = cap[1].parse::<i64>() {
            return Ok(val);
        }
    }

    // 2. Versuch: Info-Geld (info_money_account)
    let re_info = Regex::new(r"info_money_account:\s*(\d+)").unwrap();
    Ok(re_info
        .captures(&content)
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0))
}

#[command]
pub fn read_xp(
    profile_state: State<'_, AppProfileState>,
) -> Result<i64, String> {
    let path = get_active_save_path(profile_state)?;
    log!("Lese XP aus: {:?}", path);
    
    let content = decrypt_if_needed(&path)?;

    // 1. Versuch: Echte XP
    let re_main = Regex::new(r"(?m)^\s*experience_points:\s*(\d+)").unwrap();
    if let Some(cap) = re_main.captures(&content) {
        if let Ok(val) = cap[1].parse::<i64>() {
            return Ok(val);
        }
    }

    // 2. Versuch: Info-XP
    let re_info = Regex::new(r"info_players_experience:\s*(\d+)").unwrap();
    Ok(re_info
        .captures(&content)
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0))
}

#[command]
pub fn read_all_save_data(
    profile_state: State<'_, AppProfileState>,
) -> Result<SaveGameData, String> {
    let path = get_active_save_path(profile_state)?;
    log!("Lese alle Speicherdaten aus: {:?}", path);
    
    let content = decrypt_if_needed(&path)?;

    // Hilfsfunktion: Sucht erst nach Hauptwert, dann nach Info-Wert
    let find_val = |main_pat: &str, info_pat: &str| -> Option<i64> {
        let re_main = Regex::new(main_pat).unwrap();
        if let Some(cap) = re_main.captures(&content) {
            if let Ok(val) = cap[1].parse::<i64>() {
                return Some(val);
            }
        }
        let re_info = Regex::new(info_pat).unwrap();
        re_info.captures(&content).and_then(|c| c[1].parse().ok())
    };

    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = SaveGameData {
        money: find_val(r"(?m)^\s*money_account:\s*(\d+)", r"info_money_account:\s*(\d+)"),
        xp: find_val(r"(?m)^\s*experience_points:\s*(\d+)", r"info_players_experience:\s*(\d+)"),
        recruitments: re(r"info_unlocked_recruitments:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
        dealers: re(r"info_unlocked_dealers:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
        visited_cities: re(r"info_visited_cities:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
    };

    log!(
        "Gefundene Daten: Geld: {:?}, XP: {:?}, Recruitments: {:?}, dealers: {:?}, visited_cities: {:?}",
        data.money, data.xp, data.recruitments, data.dealers, data.visited_cities
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
