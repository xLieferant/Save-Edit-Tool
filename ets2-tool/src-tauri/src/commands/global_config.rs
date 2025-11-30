// Make sure these imports match your project structure
use crate::utils::paths::ets2_base_config_path;
// Import the new struct from models
use crate::models::global_config_info::BaseGameConfig; 
use crate::log; 
use tauri::command;
use regex::Regex;
use std::fs;
use std::path::Path; // Wird für path.exists() benötigt

//* Liest die globale config.cfg im Basis-Verzeichnis des Spiels *//
#[command]
pub fn read_base_config() -> Result<BaseGameConfig, String> {
    
    log!("Lese globale Config"); 

    // Pfad zur globalen config.cfg ermitteln
    let path = match ets2_base_config_path() {
        Some(p) => p,
        None => {
            let err_msg = "Konnte Standardpfad zur globalen config.cfg nicht ermitteln.";
            log!("{}", err_msg);
            return Err(err_msg.into());
        }
    };

    // Prüfe, ob die Datei existiert
    if !path.exists() {
        let err_msg = format!("Die Datei wurde nicht gefunden unter: {:?}", path);
        log!("{}", err_msg);
        return Err(err_msg);
    }

    // config.cfg wird **nicht entschlüsselt**, nur gelesen
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Fehler beim Lesen der Datei: {}", e))?;

    // Sicherstellen, dass Regex korrekt funktioniert
    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = BaseGameConfig {
        max_convoy_size: re(r#"uset g_max_convoy_size\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        traffic: re(r#"uset g_traffic\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        developer: re(r#"uset g_developer\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        console: re(r#"uset g_console\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
    };

    log!(
        "Gefundene Daten: max_convoy_size {:?}, traffic {:?}, developer {:?}, console {:?}",
        data.max_convoy_size,
        data.traffic,
        data.developer,
        data.console,
    );

    Ok(data)
}
