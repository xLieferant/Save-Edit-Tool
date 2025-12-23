use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::{autosave_path, ets2_base_config_path, quicksave_config_path};
use crate::log;
use tauri::command;
use std::env;
use regex::Regex;
use std::fs;
use serde::Deserialize;
use std::path::Path;

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
pub fn edit_level(xp: i64) -> Result<(), String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;
    // Der Befehl heißt edit_level, aber ändert die XP. Das ist konsistent mit deinem JS.
    let re = Regex::new(r"info_players_experience:\s*\d+").unwrap();
    let new = re.replace(&content, format!("info_players_experience: {}", xp));
    fs::write(&path, new.as_bytes()).map_err(|e| e.to_string())?;
    log!("XP (via edit_level) geändert: {}", xp);
    Ok(())
}

#[derive(Deserialize)]
pub struct EditValuePayload {
    value: String,
}

#[command]
pub fn edit_truck_odometer(value: i64) -> Result<(), String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;
    let path = Path::new(&profile).join("save").join("quicksave").join("game.sii");
    let content = decrypt_if_needed(&path)?;

    // Finde zuerst den Truck des Spielers
    let re_player_truck = Regex::new(r"player\s*:\s*[A-Za-z0-9._]+\s*\{[^}]*?my_truck\s*:\s*([A-Za-z0-9._]+)").unwrap();
    let player_truck_id = re_player_truck.captures(&content)
        .and_then(|c| c.get(1).map(|v| v.as_str()))
        .ok_or("Player-Truck ID nicht gefunden".to_string())?;

    // Finde den vehicle-Block des Trucks und ersetze den Odometer
    let vehicle_regex_str = format!(r"(vehicle\s*:\s*{}\s*\{{([\s\S]*?odometer:\s*)-?\d+([\s\S]*?)\}})", regex::escape(player_truck_id));
    let re_vehicle = Regex::new(&vehicle_regex_str).map_err(|e| e.to_string())?;
    
    if !re_vehicle.is_match(&content) {
        return Err(format!("Vehicle-Block für Truck {} nicht gefunden", player_truck_id));
    }

    let new_content = re_vehicle.replace(&content, format!("$1$2{}$3", value)).to_string();
    fs::write(&path, new_content).map_err(|e| e.to_string())?;
    log!("LKW-Odometer geändert: {}", value);
    Ok(())
}

#[command]
pub fn edit_truck_license_plate(value: String) -> Result<(), String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;
    let path = Path::new(&profile).join("save").join("quicksave").join("game.sii");
    let content = decrypt_if_needed(&path)?;

    let re_player_truck = Regex::new(r"player\s*:\s*[A-Za-z0-9._]+\s*\{[^}]*?my_truck\s*:\s*([A-Za-z0-9._]+)").unwrap();
    let player_truck_id = re_player_truck.captures(&content)
        .and_then(|c| c.get(1).map(|v| v.as_str()))
        .ok_or("Player-Truck ID nicht gefunden".to_string())?;

    let vehicle_regex_str = format!(r#"(vehicle\s*:\s*{}\s*\{{([\s\S]*?license_plate:\s*)"[^"]*"([\s\S]*?)\}})"#, regex::escape(player_truck_id));
    let re_vehicle = Regex::new(&vehicle_regex_str).map_err(|e| e.to_string())?;

    if !re_vehicle.is_match(&content) {
        return Err(format!("Vehicle-Block für Truck {} nicht gefunden oder hat kein Kennzeichen", player_truck_id));
    }

    let new_content = re_vehicle.replace(&content, format!("$1$2\"{}\"$3", value)).to_string();
    fs::write(&path, new_content).map_err(|e| e.to_string())?;
    log!("LKW-Kennzeichen geändert: {}", value);
    Ok(())
}

#[command]
pub fn edit_developer_value(value: i64) -> Result<(), String> {
    let path = ets2_base_config_path()
        .ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    log!("Schreibe Developer Value in: {}", path.display());

    let content = fs::read_to_string(&path)
        .map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_developer\s+"[^"]+""#).unwrap();

    if !re.is_match(&content) {
        return Err("g_developer nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(
        &content,
        format!(r#"uset g_developer "{}""#, value),
    );

    fs::write(&path, new_content.as_bytes())
        .map_err(|e| e.to_string())?;

    // Verifikation
    let verify = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !verify.contains(&format!(r#"uset g_developer "{}""#, value)) {
        return Err("Developer-Wert konnte nicht verifiziert werden".into());
    }

    log!("Dev erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_console_value(value: i64) -> Result<(), String> {
    let path = ets2_base_config_path()
        .ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    log!("Schreibe Console Value in: {}", path.display());

    let content = fs::read_to_string(&path)
        .map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_console\s+"[^"]+""#).unwrap();

    if !re.is_match(&content) {
        return Err("g_console nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(
        &content,
        format!(r#"uset g_console "{}""#, value),
    );

    fs::write(&path, new_content.as_bytes())
        .map_err(|e| e.to_string())?;

    // Verifikation
    let verify = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !verify.contains(&format!(r#"uset g_console "{}""#, value)) {
        return Err("Console-Wert konnte nicht verifiziert werden".into());
    }

    log!("Dev erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_traffic_value(value: i64) -> Result<(), String> {
    let path = ets2_base_config_path()
        .ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;

    log!("Schreibe Traffic in: {}", path.display());

    let content = fs::read_to_string(&path)
        .map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_traffic\s+"[^"]+""#).unwrap();

    if !re.is_match(&content) {
        return Err("g_traffic nicht in config.cfg gefunden".into());
    }

    let new_content = re.replace(
        &content,
        format!(r#"uset g_traffic "{}""#, value),
    );

    fs::write(&path, new_content.as_bytes())
        .map_err(|e| e.to_string())?;

    // Verifikation
    let verify = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !verify.contains(&format!(r#"uset g_traffic "{}""#, value)) {
        return Err("Traffic-Wert konnte nicht verifiziert werden".into());
    }

    log!("Traffic erfolgreich geändert auf {}", value);
    Ok(())
}

#[command]
pub fn edit_parking_doubles_value(value: i64) -> Result<(), String> {

    let profile = env::var("CURRENT_PROFILE")
        .map_err(|_| "Kein Profil geladen.".to_string())?;

    let path = quicksave_config_path(&profile);

    log!("Schreibe Parking Doubles Value in: {}", path.display());

    let content = fs::read_to_string(&path)
        .map_err(|e| e.to_string())?;

    let re = Regex::new(r#"uset g_simple_parking_doubles\s+"[^"]+""#).unwrap();

    if !re.is_match(&content) {
        return Err("uset g_simple_parking_doubles nicht in player/config.cfg gefunden".into());
    }

    let new_content = re.replace(
        &content,
        format!(r#"uset g_simple_parking_doubles "{}""#, value),
    );

    fs::write(&path, new_content.as_bytes())
        .map_err(|e| e.to_string())?;

    // Verifikation
    let verify = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    if !verify.contains(&format!(r#"uset g_simple_parking_doubles "{}""#, value)) {
        return Err("Simple Parking Doubles-Wert konnte nicht verifiziert werden".into());
    }

    log!("Parking Doubles erfolgreich geändert auf {}", value);
    Ok(())
}

#[derive(Deserialize)]
pub struct KeyValuePayload {
    key: String,     
    value: String,
}

#[command]
pub fn edit_config_value(payload: KeyValuePayload) -> Result<(), String> {
    let path = ets2_base_config_path().ok_or("Globaler Config-Pfad nicht gefunden".to_string())?;
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let re = Regex::new(&format!(r#"uset {}\s*"?.*"?"#, payload.key)).unwrap();
    let new_content = re.replace(&content, format!(r#"uset {} "{}""#, payload.key, payload.value));
    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;
    log!("Globalen Config-Wert geändert: {} -> {}", payload.key, payload.value);
    Ok(())
}

#[command]
pub fn edit_save_config_value(payload: KeyValuePayload) -> Result<(), String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;
    let path = quicksave_config_path(&profile);
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let re = Regex::new(&format!(r#"uset {}\s*"?.*"?"#, payload.key)).unwrap();
    let new_content = re.replace(&content, format!(r#"uset {} "{}""#, payload.key, payload.value));
    fs::write(&path, new_content.as_bytes()).map_err(|e| e.to_string())?;
    log!("Profil-Config-Wert geändert: {} -> {}", payload.key, payload.value);
    Ok(())
}
