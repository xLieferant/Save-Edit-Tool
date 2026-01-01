use crate::log;
use crate::state::AppProfileState;
use crate::utils::current_profile::{get_current_profile, require_current_profile};
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::{autosave_path, ets2_base_config_path};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use tauri::State;

use std::fs;
use tauri::command;

#[derive(Deserialize)]
pub struct ApplyPayload {
    pub key: String,
    pub value: Value,
}

// Hilfsfunktion: Wandelt JSON-Value (String/Number/Bool) sauber in einen String um
fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        _ => v.to_string(),
    }
}

#[command]
pub fn apply_setting(
    payload: ApplyPayload,
    profile_state: State<'_, AppProfileState>,
) -> Result<(), String> {
    let val_str = value_to_string(&payload.value);
    log!(
        "apply_setting aufgerufen: Key='{}', Value='{}'",
        payload.key,
        val_str
    );

    match payload.key.as_str() {
        // ---------------------------------------------------------------------
        // GLOBAL CONFIG (config.cfg)
        // ---------------------------------------------------------------------
        "traffic" | "g_traffic" | "developer" | "g_developer" | "console" | "g_console"
        | "max_convoy_size" | "g_max_convoy_size" => {
            // Mapping auf den echten Config-Key
            let config_key = match payload.key.as_str() {
                "traffic" => "g_traffic",
                "developer" => "g_developer",
                "console" => "g_console",
                "max_convoy_size" => "g_max_convoy_size",
                k => k,
            };

            // 1. Pfad ermitteln
            let path = ets2_base_config_path()
                .ok_or("Konnte Pfad zur globalen config.cfg nicht finden.")?;

            if !path.exists() {
                return Err(format!("Datei nicht gefunden: {:?}", path));
            }

            // 2. Datei lesen
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Fehler beim Lesen der Config: {}", e))?;

            // 3. Regex: Sucht nach 'uset KEY "WERT"'
            // Wir bauen den Regex dynamisch basierend auf dem Key
            let re_str = format!(r#"(uset {}\s*)"?[\d\.]+"?"#, regex::escape(config_key));
            let re = Regex::new(&re_str).map_err(|e| e.to_string())?;

            if !re.is_match(&content) {
                return Err(format!(
                    "Eintrag '{}' in config.cfg nicht gefunden.",
                    config_key
                ));
            }

            // 4. Ersetzen
            let replacement = format!(r#"${{1}}"{}"#, val_str);
            let new_content = re.replace(&content, replacement).to_string();

            // 5. Schreiben
            fs::write(&path, new_content)
                .map_err(|e| format!("Fehler beim Schreiben der Config: {}", e))?;

            log!(
                "Global Config '{}' erfolgreich geändert auf: {}",
                config_key,
                val_str
            );
        }

        // ---------------------------------------------------------------------
        // SAVE GAME (game.sii) - Money, XP
        // ---------------------------------------------------------------------
        "money" | "xp" => {
            // 1. Profil prüfen
            let profile = require_current_profile(profile_state)?;

            let path = autosave_path(&profile);

            // 2. Datei entschlüsseln & lesen
            let content = decrypt_if_needed(&path)?;

            // 3. Regex Muster auswählen
            let (regex_str, replacement_prefix) = match payload.key.as_str() {
                "money" => (r"info_money_account:\s*\d+", "info_money_account: "),
                "xp" => (
                    r"info_players_experience:\s*\d+",
                    "info_players_experience: ",
                ),
                _ => unreachable!(),
            };

            let re = Regex::new(regex_str).unwrap();
            if !re.is_match(&content) {
                return Err(format!(
                    "Eintrag für '{}' in game.sii nicht gefunden.",
                    payload.key
                ));
            }

            // 4. Ersetzen
            let replacement = format!("{}{}", replacement_prefix, val_str);
            let new_content = re.replace(&content, replacement).to_string();

            // 5. Schreiben
            fs::write(&path, new_content.as_bytes())
                .map_err(|e| format!("Fehler beim Schreiben des Savegames: {}", e))?;

            log!(
                "Savegame '{}' erfolgreich geändert auf: {}",
                payload.key,
                val_str
            );
        }

        // ---------------------------------------------------------------------
        // FALLBACK
        // ---------------------------------------------------------------------
        _ => {
            return Err(format!(
                "Einstellung '{}' ist in apply_setting noch nicht implementiert.",
                payload.key
            ));
        }
    }

    Ok(())
}
