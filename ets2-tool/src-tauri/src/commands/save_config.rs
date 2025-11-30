// Make sure these imports match your project structure
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::autosave_path;
// Import the new struct from models
use crate::models::save_game_config::SaveGameConfig; 
use crate::log; 
use std::env;
use regex::Regex;

//* Hiermit wird die Config.cfg in Profile/quicksave/config.cfg gelesen *//
#[tauri::command]
pub fn read_save_config() -> Result<SaveGameConfig, String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| {
        log!("Error: Kein Profil geladen."); 
        "Kein Profil geladen.".to_string()
    })?;

    log!("Lese Config aus Profil: {}", profile); 
    let path = autosave_path(&profile);
    let content = decrypt_if_needed(&path)?;

    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = SaveGameConfig {
        factor_parked: re(r" uset g_lod_factor_parked\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok()),
    };
    log!(
        "Gefundene Daten:  uset g_lod_factor_parked {:?}",
        data.factor_parked,
    );
    Ok(data)
}
