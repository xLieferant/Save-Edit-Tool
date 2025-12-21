use tauri::command;
use serde::Deserialize;
use std::fs;
use regex::Regex;
use std::path::Path;

use crate::utils::paths::{
    autosave_path_current,
    base_config_path
};
use crate::utils::decrypt::{decrypt_if_needed, backup_file, modify_block};
use crate::log;

#[derive(Deserialize)]
pub struct ApplyPayload {
    pub key: String,
    pub value: serde_json::Value,
}

#[command]
pub fn apply_setting(payload: ApplyPayload) -> Result<(), String> {
    // Ermitteln, welche Datei und welchen Key wir ändern
    let path = match payload.key.as_str() {
        "money" | "xp" => autosave_path_current()?,
        "traffic" | "developer" | "console" => base_config_path()?,
        _ => return Err(format!("Unknown setting key: {}", payload.key)),
    };

    // Backup erstellen
    backup_file(&path)?;

    // Temp-Datei + Block-Modifying-Pattern
    let (block_name, regex_pattern, replacement) = match payload.key.as_str() {
        // ---------------- PROFILE / AUTOSAVE ----------------
        "money" => ("player", r"info_money_account:\s*\d+", format!("info_money_account: {}", payload.value)),
        "xp" => ("player", r"info_players_experience:\s*\d+", format!("info_players_experience: {}", payload.value)),

        // ---------------- BASE CONFIG ----------------
        "traffic" => ("config", r"traffic:\s*\d+", format!("traffic: {}", payload.value)),
        "developer" => ("config", r"developer:\s*\d+", format!("developer: {}", payload.value)),
        "console" => ("config", r"console:\s*\d+", format!("console: {}", payload.value)),

        _ => return Err(format!("Unknown setting key: {}", payload.key)),
    };

    // modify_block nutzen
    modify_block(&path, block_name, |block| {
        // Regex im Block ersetzen
        let re = Regex::new(regex_pattern).unwrap();
        re.replace(block, replacement.as_str()).to_string()
    })?;

    log!("apply_setting OK → {}", payload.key);
    Ok(())
}


// use crate::utils::decrypt::{modify_block, backup_file};
// use crate::utils::paths::quicksave_game_path;
// use crate::utils::hex_float::{float_to_hex, parse_value_auto};
// use std::env;
// use crate::log;

// /// Payload aus JS
// pub struct ApplyPayload {
//     pub key: String,
//     pub value: String, // UI liefert String
// }

// pub fn apply_setting(payload: ApplyPayload) -> Result<(), String> {
//     // 1. Profil & Pfad
//     let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil gesetzt")?;
//     let path = quicksave_game_path(&profile);

//     // 2. Backup
//     backup_file(&path)?;

//     // 3. Updater-Funktion für modify_block
//     let updater = |block_content: &str| -> String {
//         let mut new_block = block_content.to_string();

//         match payload.key.as_str() {
//             "money" => {
//                 let hex = float_to_hex(parse_value_auto(&payload.value).unwrap_or(0.0));
//                 let re = regex::Regex::new(r"money_account:\s*(&?[0-9a-fA-F]+)").unwrap();
//                 new_block = re.replace(&new_block, format!("money_account: {}", hex)).to_string();
//             }
//             "xp" => {
//                 let re = regex::Regex::new(r"experience_points:\s*(\d+)").unwrap();
//                 new_block = re.replace(&new_block, format!("experience_points: {}", payload.value)).to_string();
//             }
//             "odometer" => {
//                 let hex = float_to_hex(parse_value_auto(&payload.value).unwrap_or(0.0));
//                 let re = regex::Regex::new(r"odometer:\s*(&?[0-9a-fA-F]+)").unwrap();
//                 new_block = re.replace(&new_block, format!("odometer: {}", hex)).to_string();
//             }
//             // ... weitere Keys wie trip_fuel_l, trailer_odometer etc.
//             _ => {}
//         }

//         new_block
//     };

//     // 4. modify_block für Player oder Vehicle Block aufrufen
//     modify_block(&path, "player", updater)?;
//     // optional auch Vehicle/Trailer-Block: modify_block(&path, "vehicle", updater)?

//     log!("apply_setting erfolgreich: {}", payload.key);
//     Ok(())
// }
