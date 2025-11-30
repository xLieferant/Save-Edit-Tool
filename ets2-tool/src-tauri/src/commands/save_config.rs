// Make sure these imports match your project structure
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::quicksave_config_path;
// Import the new struct from models
use crate::models::save_game_config::SaveGameConfig; 
use crate::log; 
use tauri::command;
use regex::Regex;
use std::path::Path; // Wird für path.exists() benötigt, war vermutlich schon da

//* Hiermit wird die config.cfg im Basis-Ordner des Profils gelesen (z.B. .../Euro Truck Simulator 2/profiles/12345/config.cfg) *//
#[command]
// Der Funktionskopf akzeptiert den Pfad direkt
pub fn read_save_config(profile_path: &str) -> Result<SaveGameConfig, String> {
    
    // Die Logik für env::var("CURRENT_PROFILE") entfällt hier komplett.
    // Wir nehmen den übergebenen profile_path direkt.
    
    log!("Lese Config aus Profilpfad: {}", profile_path); 

    // Verwende die korrekte Hilfsfunktion, um den vollen Pfad zur config.cfg innerhalb des Profilordners zu erhalten
    let path = quicksave_config_path(profile_path); 
    
    // Prüfe, ob die Datei existiert, bevor du versuchst, sie zu entschlüsseln/lesen
    if !path.exists() {
        let err_msg = format!("Error: Die Datei wurde nicht gefunden unter: {:?}", path);
        log!("{}", err_msg);
        return Err(err_msg);
    }

    let content = decrypt_if_needed(&path)?;

    // Sicherstellen, dass Regex korrekt funktioniert
    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = SaveGameConfig {
        // Der Regex-Pattern sucht nach dem genauen String in der config.cfg
        // c[1] ist korrekt für die erste Capture-Gruppe (die Ziffern).
        factor_parked: re(r#"uset g_lod_factor_parked\s*"(\d+)""#).captures(&content).and_then(|c| c[1].parse().ok()),
    };
    log!(
        "Gefundene Daten: uset g_lod_factor_parked {:?}",
        data.factor_parked,
    );
    Ok(data)
}
