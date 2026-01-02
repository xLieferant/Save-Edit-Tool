// Make sure these imports match your project structure
use crate::shared::paths::ets2_base_config_path;
// Import the new struct from models
use crate::dev_log;
use crate::models::global_config_info::BaseGameConfig;
use regex::Regex;
use std::fs;
use tauri::command;
// Wird für path.exists() benötigt
use crate::shared::paths::quicksave_config_path;
use crate::shared::decrypt::decrypt_if_needed;
use crate::models::save_game_config::SaveGameConfig;

//* Liest die globale config.cfg im Basis-Verzeichnis des Spiels *//
#[command]
pub fn read_base_config() -> Result<BaseGameConfig, String> {
    dev_log!("Lese globale Config");

    // Pfad zur globalen config.cfg ermitteln
    let path = match ets2_base_config_path() {
        Some(p) => p,
        None => {
            let err_msg = "Konnte Standardpfad zur globalen config.cfg nicht ermitteln.";
            dev_log!("{}", err_msg);
            return Err(err_msg.into());
        }
    };

    // Prüfe, ob die Datei existiert
    if !path.exists() {
        let err_msg = format!("Die Datei wurde nicht gefunden unter: {:?}", path);
        dev_log!("{}", err_msg);
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

    dev_log!(
        "Gefundene Daten: max_convoy_size {:?}, traffic {:?}, developer {:?}, console {:?}",
        data.max_convoy_size,
        data.traffic,
        data.developer,
        data.console,
    );

    Ok(data)
}

//* Hiermit wird die config.cfg im Basis-Ordner des Profils gelesen (z.B. .../Euro Truck Simulator 2/profiles/12345/config.cfg) *//
#[command]
// Der Funktionskopf akzeptiert den Pfad direkt
pub fn read_save_config(profile_path: &str) -> Result<SaveGameConfig, String> {
    // Wir nehmen den übergebenen profile_path direkt.

    dev_log!("Lese Config aus Profilpfad: {}", profile_path);

    // Verwende die korrekte Hilfsfunktion, um den vollen Pfad zur config.cfg innerhalb des Profilordners zu erhalten
    let path = quicksave_config_path(profile_path);

    // Prüfe, ob die Datei existiert, bevor du versuchst, sie zu entschlüsseln/lesen
    if !path.exists() {
        let err_msg = format!("Error: Die Datei wurde nicht gefunden unter: {:?}", path);
        dev_log!("{}", err_msg);
        return Err(err_msg);
    }

    let content = decrypt_if_needed(&path)?;

    // Sicherstellen, dass Regex korrekt funktioniert
    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = SaveGameConfig {
        // Der Regex-Pattern sucht nach dem genauen String in der config.cfg
        // c[1] ist korrekt für die erste Capture-Gruppe (die Ziffern).
        factor_parking_doubles: re(r#"uset g_simple_parking_doubles\s*"?(\d+)"??"#)
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
    };
    dev_log!(
        "Gefundene Daten: uset g_simple_parking_doubles {:?}",
        data.factor_parking_doubles,
    );
    Ok(data)
}
