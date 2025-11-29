use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::autosave_path;
use crate::models::save_game_data::SaveGameData;
use crate::log; // This import is now used
use tauri::command;
use std::env;
use regex::Regex;

#[command]
pub fn read_money() -> Result<i64, String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| {
        log!("Error: Kein Profil geladen."); // Used here
        "Kein Profil geladen.".to_string()
    })?;
    
    log!("Lese Geld aus Profil: {}", profile); // Used here
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(r"info_money_account:\s*(\d+)").unwrap();
    Ok(re.captures(&content).and_then(|c| c[1].parse().ok()).unwrap_or(0))
}

#[command]
pub fn read_xp() -> Result<i64, String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| {
        log!("Error: Kein Profil geladen."); // Used here
        "Kein Profil geladen.".to_string()
    })?;
    
    log!("Lese XP aus Profil: {}", profile); // Used here
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(r"info_players_experience:\s*(\d+)").unwrap();
    Ok(re.captures(&content).and_then(|c| c[1].parse().ok()).unwrap_or(0))
}

#[command]
pub fn read_all_save_data() -> Result<SaveGameData, String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| {
        log!("Error: Kein Profil geladen."); // Used here
        "Kein Profil geladen.".to_string()
    })?;

    log!("Lese alle Speicherdaten aus Profil: {}", profile); // Used here
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;

    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = SaveGameData {
        money: re(r"info_money_account:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
        xp: re(r"info_players_experience:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
        level: re(r"info_player_level:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
        garages: re(r"garages:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
        trucks_owned: re(r"trucks:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
        trailers_owned: re(r"trailers:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
        kilometers_total: re(r"km_total:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
    };
    log!("Daten erfolgreich gelesen."); // Used here
    log!("Daten erfolgreich gelesen: Geld: {:?}, XP: {:?}", data.money, data.xp);
    Ok(data)
}
