use crate::dev_log;
use crate::state::{AppProfileState, DecryptCache};
use crate::shared::current_profile::{get_current_profile, require_current_profile};
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::paths::{
    autosave_path, ets2_base_config_path, game_sii_from_save, quicksave_config_path,
    quicksave_game_path,
};
use crate::shared::sii_parser;
use crate::shared::regex_helper::cragex;
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use tauri::State;
use tauri::command;

fn get_active_save_path(
    profile_state: State<'_, AppProfileState>,
) -> Result<std::path::PathBuf, String> {
    let save_opt = profile_state.current_save.lock().unwrap().clone();
    if let Some(save) = save_opt {
        return Ok(game_sii_from_save(Path::new(&save)));
    }
    let profile = require_current_profile(profile_state)?;
    Ok(autosave_path(&profile))
}

#[command]
pub fn edit_money(amount: i64, profile_state: State<'_, AppProfileState>) -> Result<(), String> {
    let path = get_active_save_path(profile_state)?;
    let content = decrypt_if_needed(&path)?;

    // 1. Info-Wert ersetzen
    let re_info = Regex::new(r"info_money_account:\s*\d+").unwrap();
    let content = re_info
        .replace(&content, format!("info_money_account: {}", amount))
        .to_string();

    // 2. Echten Wert ersetzen (unter Beibehaltung der Einrückung)
    let re_main = Regex::new(r"(?m)^(\s*)money_account:\s*\d+").unwrap();
    let content = re_main
        .replace(&content, format!("${{1}}money_account: {}", amount))
        .to_string();

    fs::write(&path, content.as_bytes()).map_err(|e| e.to_string())?;
    dev_log!("Geld geändert: {}", amount);
    Ok(())
}

#[command]
pub fn edit_xp(xp: i64, profile_state: State<'_, AppProfileState>) -> Result<(), String> {
    let path = get_active_save_path(profile_state)?;
    let content = decrypt_if_needed(&path)?;

    // 1. Info-Wert ersetzen
    let re_info = Regex::new(r"info_players_experience:\s*\d+").unwrap();
    let content = re_info
        .replace(&content, format!("info_players_experience: {}", xp))
        .to_string();

    // 2. Echten Wert ersetzen
    let re_main = Regex::new(r"(?m)^(\s*)experience_points:\s*\d+").unwrap();
    let content = re_main
        .replace(&content, format!("${{1}}experience_points: {}", xp))
        .to_string();

    fs::write(&path, content.as_bytes()).map_err(|e| e.to_string())?;
    dev_log!("XP geändert: {}", xp);
    Ok(())
}

#[command]
pub fn edit_level(xp: i64, profile_state: State<'_, AppProfileState>) -> Result<(), String> {
    let path = get_active_save_path(profile_state)?;
    let content = decrypt_if_needed(&path)?;

    // 1. Info-Wert ersetzen
    let re_info = Regex::new(r"info_players_experience:\s*\d+").unwrap();
    let content = re_info
        .replace(&content, format!("info_players_experience: {}", xp))
        .to_string();

    // 2. Echten Wert ersetzen
    let re_main = Regex::new(r"(?m)^(\s*)experience_points:\s*\d+").unwrap();
    let content = re_main
        .replace(&content, format!("${{1}}experience_points: {}", xp))
        .to_string();

    fs::write(&path, content.as_bytes()).map_err(|e| e.to_string())?;
    dev_log!("XP (via edit_level) geändert: {}", xp);
    Ok(())
}

#[derive(Deserialize)]
pub struct EditValuePayload {
    value: String,
}

#[command]
pub fn edit_player_money(
    value: i64,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    dev_log!("--- edit_player_money START ---");

    // ✅ Use the helper - respects current_save if set
    let path = get_active_save_path(profile_state)?;
    let content = decrypt_if_needed(&path)?;

    let re_money = Regex::new(r"money_account:\s*(\d+)").map_err(|e| e.to_string())?;

    if !re_money.is_match(&content) {
        return Err("money_account nicht gefunden".into());
    }

    let new_content = re_money.replace(&content, format!("money_account: {}", value));

    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;

    dev_log!("Money erfolgreich geändert auf {}", value);
    dev_log!("--- edit_player_money END ---");

    Ok(())
}

#[command]
pub fn edit_player_experience(
    value: i64,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    dev_log!("--- edit_player_experience START ---");

    // ✅ Use the helper - respects current_save if set
    let path = get_active_save_path(profile_state)?;
    let content = decrypt_if_needed(&path)?;

    let re_experience = Regex::new(r"experience_points:\s*(\d+)").map_err(|e| e.to_string())?;

    if !re_experience.is_match(&content) {
        return Err("experience_points: nicht gefunden".into());
    }

    let new_content = re_experience.replace(&content, format!("experience_points: {}", value));

    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;

    dev_log!("Experience erfolgreich geändert auf {}", value);
    dev_log!("--- edit_player_experience END ---");

    Ok(())
}

#[command]
pub fn edit_skill_value(
    skill: String,
    value: i64,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    dev_log!("--- edit_skill START ---");
    dev_log!("Skill: {}, Wert: {}", skill, value);

    // ✅ Use the helper - respects current_save if set
    let path = get_active_save_path(profile_state)?;
    let content = decrypt_if_needed(&path)?;

    // Regex dynamisch je Skill
    let re = Regex::new(&format!(r"\b{}\s*:\s*\d+", regex::escape(&skill)))
        .map_err(|e| e.to_string())?;

    if !re.is_match(&content) {
        return Err(format!("Skill '{}' nicht gefunden", skill));
    }

    let new_content = re.replace(&content, format!("{}: {}", skill, value));

    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;

    dev_log!("Skill '{}' erfolgreich geändert auf {}", skill, value);
    dev_log!("--- edit_skill END ---");

    Ok(())
}



#[command]
pub fn edit_developer_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    dev_log!("Schreibe Developer Value in: {}", path.display());

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_developer\s+"[^"]+""#).unwrap();

    if !re.is_match(&content) {
        return Err("g_developer nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(&content, format!(r#"uset g_developer "{}""#, value));

    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;

    // Verifikation
    let verify = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !verify.contains(&format!(r#"uset g_developer "{}""#, value)) {
        return Err("Developer-Wert konnte nicht verifiziert werden".into());
    }

    dev_log!("Dev erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_console_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    dev_log!("Schreibe Console Value in: {}", path.display());

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_console\s+"[^"]+""#).unwrap();

    if !re.is_match(&content) {
        return Err("g_console nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(&content, format!(r#"uset g_console "{}""#, value));

    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;

    // Verifikation
    let verify = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !verify.contains(&format!(r#"uset g_console "{}""#, value)) {
        return Err("Console-Wert konnte nicht verifiziert werden".into());
    }

    dev_log!("Dev erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_convoy_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    dev_log!("Schreibe Convoy in: {}", path.display());

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_max_convoy_size\s+"[^"]+""#).unwrap();

    if !re.is_match(&content) {
        return Err("g_max_convoy_size nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(&content, format!(r#"uset g_max_convoy_size "{}""#, value));

    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;

    // Verifikation
    let verify = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !verify.contains(&format!(r#"uset g_max_convoy_size "{}""#, value)) {
        return Err("Convoy-Wert konnte nicht verifiziert werden".into());
    }

    dev_log!("Convoy erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_traffic_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    // 🔒 Clamping: garantiert 0–10
    let value = value.clamp(0, 10);

    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    dev_log!("Schreibe Traffic in: {} (Wert: {})", path.display(), value);

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_traffic\s+"[^"]+""#).map_err(|e| e.to_string())?;

    if !re.is_match(&content) {
        return Err("g_traffic nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(&content, format!(r#"uset g_traffic "{}""#, value));

    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;

    dev_log!("Traffic erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_parking_doubles_value(
    value: i64,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    let profile = require_current_profile(profile_state)?;

    let path = quicksave_config_path(&profile);

    dev_log!("Schreibe Parking Doubles Value in: {}", path.display());

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_simple_parking_doubles\s+"[^"]+""#).unwrap();

    if !re.is_match(&content) {
        return Err("uset g_simple_parking_doubles nicht in player/config.cfg gefunden".into());
    }

    let new_content = re.replace(
        &content,
        format!(r#"uset g_simple_parking_doubles "{}""#, value),
    );

    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;

    // Verifikation
    let verify = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !verify.contains(&format!(r#"uset g_simple_parking_doubles "{}""#, value)) {
        return Err("Simple Parking Doubles-Wert konnte nicht verifiziert werden".into());
    }

    dev_log!("Parking Doubles erfolgreich geändert auf {}", value);
    Ok(())
}

#[derive(Deserialize)]
pub struct KeyValuePayload {
    key: String,
    value: String,
}

#[command]
pub fn edit_config_value(
    payload: KeyValuePayload,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let re = Regex::new(&format!(r#"uset {}\s*"?.*"?"#, payload.key)).unwrap();
    let new_content = re.replace(
        &content,
        format!(r#"uset {} "{}""#, payload.key, payload.value),
    );
    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;
    dev_log!(
        "Globalen Config-Wert geändert: {} -> {}",
        payload.key,
        payload.value
    );
    Ok(())
}

#[command]
pub fn edit_save_config_value(
    payload: KeyValuePayload,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    let profile = require_current_profile(profile_state)?;
    let path = quicksave_config_path(&profile);
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let re = Regex::new(&format!(r#"uset {}\s*"?.*"?"#, payload.key)).unwrap();
    let new_content = re.replace(
        &content,
        format!(r#"uset {} "{}""#, payload.key, payload.value),
    );
    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;
    dev_log!(
        "Profil-Config-Wert geändert: {} -> {}",
        payload.key,
        payload.value
    );
    Ok(())
}
