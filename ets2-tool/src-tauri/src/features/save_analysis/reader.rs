use crate::dev_log;
use crate::shared::user_log;
use crate::models::save_game_data::SaveGameData;
use crate::shared::decrypt::decrypt_cached_with_cache;
use crate::shared::paths::{autosave_path, ets2_base_config_path, info_sii_from_save};
use crate::shared::trace::TraceScope;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use tauri::command;

fn get_active_save_path_from_snapshot(
    current_save: Option<String>,
    current_profile: Option<String>,
) -> Result<PathBuf, String> {
    if let Some(save) = current_save {
        return Ok(info_sii_from_save(Path::new(&save)));
    }
    let profile = current_profile.ok_or_else(|| "Kein Profil geladen.".to_string())?;
    Ok(autosave_path(&profile))
}

// #[command]
// pub fn read_money(profile_state: State<'_, AppProfileState>) -> Result<i64, String> {
//     let path = get_active_save_path(profile_state)?;
//     dev_log!("Lese Geld aus: {:?}", path);

//     let content = decrypt_if_needed(&path)?;

//     // 1. Versuch: Echtes Geld (money_account)
//     // (?m)^\s* verhindert, dass wir "info_money_account" matchen
//     let re_main = Regex::new(r"(?m)^\s*money_account:\s*(\d+)").unwrap();
//     if let Some(cap) = re_main.captures(&content) {
//         if let Ok(val) = cap[1].parse::<i64>() {
//             return Ok(val);
//         }
//     }

//     // 2. Versuch: Info-Geld (info_money_account)
//     let re_info = Regex::new(r"info_money_account:\s*(\d+)").unwrap();
//     Ok(re_info
//         .captures(&content)
//         .and_then(|c| c[1].parse().ok())
//         .unwrap_or(0))
// }

// #[command]
// pub fn read_xp(profile_state: State<'_, AppProfileState>) -> Result<i64, String> {
//     let path = get_active_save_path(profile_state)?;
//     dev_log!("Lese XP aus: {:?}", path);

//     let content = decrypt_if_needed(&path)?;

//     // 1. Versuch: Echte XP
//     let re_main = Regex::new(r"(?m)^\s*experience_points:\s*(\d+)").unwrap();
//     if let Some(cap) = re_main.captures(&content) {
//         if let Ok(val) = cap[1].parse::<i64>() {
//             return Ok(val);
//         }
//     }

//     // 2. Versuch: Info-XP
//     let re_info = Regex::new(r"info_players_experience:\s*(\d+)").unwrap();
//     Ok(re_info
//         .captures(&content)
//         .and_then(|c| c[1].parse().ok())
//         .unwrap_or(0))
// }

#[command]
pub async fn read_all_save_data(
    profile_state: State<'_, AppProfileState>,
    cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<SaveGameData, String> {
    let mut trace = TraceScope::new("read_all_save_data");
    let current_save = profile_state
        .current_save
        .lock()
        .map_err(|_| "AppProfileState current_save lock poisoned".to_string())?
        .clone();
    let current_profile = profile_state
        .current_profile
        .lock()
        .map_err(|_| "AppProfileState current_profile lock poisoned".to_string())?
        .clone();
    let path = get_active_save_path_from_snapshot(current_save, current_profile)?;
    let path_key = path.display().to_string();

    if let Some(cached) = cache.get_save_game_data(&path_key) {
        dev_log!("Lese alle Speicherdaten aus Cache: {:?}", path);
        trace.finish_ok();
        return Ok(cached);
    }

    dev_log!("Lese alle Speicherdaten aus: {:?}", path);
    let decrypt_cache = decrypt_cache.inner().clone();
    let path_for_worker = path.clone();
    let data = tauri::async_runtime::spawn_blocking(move || {
        read_all_save_data_from_path(path_for_worker, &decrypt_cache)
    })
    .await
    .map_err(|error| format!("read_all_save_data join failed: {}", error))??;

    // Hilfsfunktion: Sucht erst nach Hauptwert, dann nach Info-Wert
    dev_log!(
        "Gefundene Daten: Geld: {:?}, XP: {:?}, Recruitments: {:?}, dealers: {:?}, visited_cities: {:?}",
        data.money,
        data.xp,
        data.recruitments,
        data.dealers,
        data.visited_cities
    );
    let _ = user_log::user_log_info("SaveEditor", "Save data read from the active save.");

    cache.cache_save_game_data(path_key, data.clone());
    trace.finish_ok();
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

fn read_all_save_data_from_path(
    path: PathBuf,
    decrypt_cache: &DecryptCache,
) -> Result<SaveGameData, String> {
    let content = decrypt_cached_with_cache(&path, decrypt_cache)?;
    let mut parser_trace = TraceScope::with_fields(
        "read_all_save_data parser",
        &[("path", path.display().to_string())],
    );

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
        money: find_val(
            r"(?m)^\s*money_account:\s*(\d+)",
            r"info_money_account:\s*(\d+)",
        ),
        xp: find_val(
            r"(?m)^\s*experience_points:\s*(\d+)",
            r"info_players_experience:\s*(\d+)",
        ),
        recruitments: re(r"info_unlocked_recruitments:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        dealers: re(r"info_unlocked_dealers:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        visited_cities: re(r"(?i)info_visited_cities:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
    };
    parser_trace.finish_ok();
    Ok(data)
}
