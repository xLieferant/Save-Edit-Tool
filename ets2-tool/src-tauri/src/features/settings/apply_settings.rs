use crate::dev_log;
use crate::features::backup::service as backup_service;
use crate::features::logging::service as logging_service;
use crate::shared::decrypt::decrypt_if_needed;
use crate::state::{AppProfileState, ProfileCache};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;
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

fn write_text_with_auto_backup(
    profile_state: &AppProfileState,
    path: &Path,
    action: &str,
    action_reason: &str,
    success_message: &str,
    content: &str,
    before: &str,
) -> Result<(), String> {
    let _mutation = crate::features::save_editor::safety::mutation_lock()?;
    let raw = crate::features::save_editor::safety::read_source(profile_state, path)?;
    if crate::shared::decrypt::decode_text_bytes(&raw, "active_setting", &[])? != before {
        return Err("save_changed_since_load".into());
    }
    let mut context = logging_service::resolve_active_context(profile_state);
    context.extra.insert(
        "target".to_string(),
        logging_service::redact_path(&path.display().to_string()),
    );
    context
        .extra
        .insert("reason".to_string(), action_reason.to_string());

    let backup = backup_service::create_backup_for_targets(
        profile_state,
        action_reason,
        &backup_service::recommended_targets(path),
    )
    .map_err(|error| {
        let _ = logging_service::record_error(
            action,
            Some("auto_backup_failed"),
            "Automatic backup could not be created before the setting was applied.",
            Some(&error),
            &context,
        );
        "Automatisches Backup konnte vor dem Anwenden der Einstellung nicht erstellt werden."
            .to_string()
    })?;

    context
        .extra
        .insert("backupId".to_string(), backup.backup_id);
    crate::features::save_editor::safety::write_atomic_checked(path, &raw, content, |_| Ok(()))?;
    if path.file_name().is_some_and(|name| name == "game.sii") {
        crate::features::save_editor::safety::remember_save(profile_state, path)?;
    }

    let _ = logging_service::record_info(action, success_message, &context);
    Ok(())
}

#[cfg_attr(not(test), command(async))]
#[cfg_attr(test, command)]
pub fn apply_setting(
    payload: ApplyPayload,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
) -> Result<(), String> {
    let val_str = value_to_string(&payload.value);
    dev_log!(
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
            let path = crate::features::save_editor::safety::config_path(profile_state.inner())?;

            if !path.exists() {
                return Err(format!("Datei nicht gefunden: {:?}", path));
            }

            // 2. Datei lesen
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Fehler beim Lesen der Config: {}", e))?;

            let new_content =
                super::game_config::patch_config_value(&content, config_key, &val_str)?;

            // 5. Schreiben
            write_text_with_auto_backup(
                profile_state.inner(),
                &path,
                "apply_setting_global_config",
                &format!("before apply setting {}", config_key),
                "A global config setting was updated.",
                &new_content,
                &content,
            )?;

            profile_cache.invalidate_base_config();
            dev_log!(
                "Global Config '{}' erfolgreich geändert auf: {}",
                config_key,
                val_str
            );
        }

        // ---------------------------------------------------------------------
        // SAVE GAME (game.sii) - Money, XP
        // ---------------------------------------------------------------------
        "money" | "xp" => {
            let path = crate::features::save_editor::safety::active_save(profile_state.inner())?;
            let raw =
                crate::features::save_editor::safety::read_source(profile_state.inner(), &path)?;
            let content = crate::shared::decrypt::decode_text_bytes(&raw, "setting_edit", &[])?;
            let value = val_str
                .parse::<i64>()
                .map_err(|_| "setting_value_invalid")?;
            let new_content = if payload.key == "money" {
                crate::features::save_editor::document::patch_money(&content, value)?
            } else {
                crate::features::save_editor::coaches::patch_xp(
                    &content,
                    crate::features::save_editor::coaches::Career::Truck,
                    value,
                )?
            };

            // 5. Schreiben
            write_text_with_auto_backup(
                profile_state.inner(),
                &path,
                "apply_setting_save",
                &format!("before apply setting {}", payload.key),
                "A save setting was updated.",
                &new_content,
                &content,
            )?;

            profile_cache.invalidate_save_data();
            dev_log!(
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
