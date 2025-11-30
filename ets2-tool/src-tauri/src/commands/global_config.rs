use crate::log;
use crate::models::global_config_info::BaseGameConfig; // <- Korrigiert: richtiges Modul importieren
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::ets2_base_config_path;
use regex::Regex;
use std::fs;
use tauri::command;

/// Liest die Basis-Config (config.cfg) des Spiels
#[command]
pub fn read_base_config() -> Result<BaseGameConfig, String> {
    log!("Lese globale Config");

    // Pfad ermitteln
    let path = match ets2_base_config_path() {
        Some(p) => p,
        None => {
            let err_msg = "Konnte Standardpfad zur config.cfg nicht ermitteln.";
            log!("{}", err_msg);
            return Err(err_msg.into());
        }
    };

    // Existenz prüfen
    if !path.exists() {
        let err_msg = format!("Die Datei wurde nicht gefunden unter: {:?}", path);
        log!("{}", err_msg);
        return Err(err_msg);
    }

    // config.cfg **nicht entschlüsseln**, nur lesen
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Fehler beim Lesen der Datei: {}", e))?;

    // Regex zum Auslesen der Werte
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
