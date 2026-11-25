use crate::dev_log;
use crate::models::save_game_data::SaveGameData;
use crate::shared::decrypt::decrypt_cached_with_cache;
use crate::shared::trace::TraceScope;
use crate::shared::user_log;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use regex::Regex;
use std::fs;
use std::path::PathBuf;
use tauri::State;
use tauri::command;

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
    let path = crate::features::save_editor::safety::active_save(profile_state.inner())?;
    let path_key = path.display().to_string();

    if let Some(cached) = cache.get_save_game_data(&path_key) {
        dev_log!("Lese alle Speicherdaten aus Cache: {:?}", path);
        trace.finish_ok();
        return Ok(cached);
    }

    dev_log!("Lese alle Speicherdaten aus: {:?}", path);
    let read_revision =
        crate::features::save_editor::safety::read_revision(profile_state.inner(), &path)?;
    let decrypt_cache = decrypt_cache.inner().clone();
    let path_for_worker = path.clone();
    let data = tauri::async_runtime::spawn_blocking(move || {
        read_all_save_data_from_path(path_for_worker, &decrypt_cache)
    })
    .await
    .map_err(|error| format!("read_all_save_data join failed: {}", error))??;
    let _publication = crate::features::save_editor::safety::mutation_lock()?;
    crate::features::save_editor::safety::ensure_read_current(
        profile_state.inner(),
        &read_revision,
    )?;

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
pub fn read_traffic_value(profile_state: State<'_, AppProfileState>) -> Result<i64, String> {
    let path = crate::features::save_editor::safety::config_path(profile_state.inner())?;
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

pub(crate) fn read_all_save_data_from_path(
    path: PathBuf,
    decrypt_cache: &DecryptCache,
) -> Result<SaveGameData, String> {
    let content = decrypt_cached_with_cache(&path, decrypt_cache)?;
    let mut parser_trace = TraceScope::with_fields(
        "read_all_save_data parser",
        &[("path", path.display().to_string())],
    );

    let doc = crate::features::save_editor::document::Document::parse(&content)?;
    let money = doc
        .field(doc.bank()?, "money_account")?
        .parse::<i64>()
        .map_err(|_| "money_invalid")?;
    let xp = doc
        .field(doc.unique("economy")?, "experience_points")?
        .parse::<i64>()
        .map_err(|_| "xp_invalid")?;
    // info.sii only supplies descriptive counters. It never supplies editable values.
    let info = path.with_file_name("info.sii");
    let content = if info.is_file() {
        decrypt_cached_with_cache(&info, decrypt_cache).unwrap_or_default()
    } else {
        String::new()
    };

    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = SaveGameData {
        money: Some(money),
        xp: Some(xp),
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

#[cfg(test)]
mod ats_tests {
    #[test]
    fn ats_values_come_from_game_even_when_metadata_disagrees() {
        let dir = std::env::temp_dir().join(format!("ats-reader-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("game.sii"),
            crate::features::save_editor::ats_tests::fixture(),
        )
        .unwrap();
        std::fs::write(dir.join("info.sii"), "SiiNunit\n{\nsave_container : save.main {\n info_money_account: 1500\n info_players_experience: 7\n}\n}\n").unwrap();
        let data = super::read_all_save_data_from_path(
            dir.join("game.sii"),
            &crate::state::DecryptCache::default(),
        )
        .unwrap();
        assert_eq!(data.money, Some(6500));
        assert_eq!(data.xp, Some(43863));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
