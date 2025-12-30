use crate::log;
use crate::models::cached_profile::CachedProfile;
use crate::models::profile_info::ProfileInfo;
use crate::models::save_info::SaveInfo;
use crate::state::{AppProfileState, DecryptCache};
use crate::utils::current_profile::set_current_profile;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::extract::extract_profile_name;
use crate::utils::extract_save_name::extract_save_name;
use crate::models::profile_info::SaveKind;
use crate::utils::hex::decode_hex_folder_name;
use crate::utils::paths::ets2_base_path;
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::command;
use tauri::Manager;
use tauri::State;

use std::io::Write;
use std::path::PathBuf;
// use std::path::Path;

fn app_cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    // tauri::api::path::config_dir() liefert Option<PathBuf> -> v2: app.path().config_dir()
    if let Ok(mut dir) = app.path().config_dir() {
        dir.push("save-edit-tool"); // eigener Ordner für die App
        if let Err(e) = fs::create_dir_all(&dir) {
            return Err(format!("Failed to create config dir: {}", e));
        }
        Ok(dir)
    } else {
        Err("Konnte Config-Verzeichnis nicht bestimmen".into())
    }
}

#[command]
pub fn save_profiles_cache(
    app: tauri::AppHandle,
    profiles: Vec<CachedProfile>,
) -> Result<String, String> {
    let dir = app_cache_dir(&app)?;
    let file = dir.join("profiles_cache.json");
    let json =
        serde_json::to_string_pretty(&profiles).map_err(|e| format!("Serialize error: {}", e))?;
    fs::write(&file, json).map_err(|e| format!("Write error: {}", e))?;
    Ok(file.display().to_string())
}

#[command]
pub fn read_profiles_cache(app: tauri::AppHandle) -> Result<Vec<CachedProfile>, String> {
    let dir = app_cache_dir(&app)?;
    let file = dir.join("profiles_cache.json");
    if !file.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&file).map_err(|e| format!("Read error: {}", e))?;
    let profiles: Vec<CachedProfile> =
        serde_json::from_str(&content).map_err(|e| format!("Parse error: {}", e))?;
    Ok(profiles)
}

#[command]
pub fn save_last_profile(app: tauri::AppHandle, profile_path: String) -> Result<String, String> {
    let dir = app_cache_dir(&app)?;
    let file = dir.join("last_profile.json");
    let obj = serde_json::json!({ "last_profile": profile_path });
    let json = serde_json::to_string_pretty(&obj).map_err(|e| format!("Serialize err: {}", e))?;
    fs::write(&file, json).map_err(|e| format!("Write err: {}", e))?;
    Ok(file.display().to_string())
}

#[command]
pub fn read_last_profile(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let dir = app_cache_dir(&app)?;
    let file = dir.join("last_profile.json");
    if !file.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&file).map_err(|e| format!("Read error: {}", e))?;
    let v: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Parse error: {}", e))?;
    if let Some(p) = v.get("last_profile").and_then(|s| s.as_str()) {
        Ok(Some(p.to_string()))
    } else {
        Ok(None)
    }
}

pub fn set_active_profile(
    profile_path: String,
    profile_state: State<'_, AppProfileState>,
    cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    // Profil setzen
    *profile_state.current_profile.lock().unwrap() = Some(profile_path.clone());

    // 🔥 Cache vollständig leeren
    cache.files.lock().unwrap().clear();

    log!(
        "Aktives Profil gesetzt & DecryptCache geleert: {}",
        profile_path
    );
    Ok(())
}

#[tauri::command]
pub fn switch_profile(
    cache: State<DecryptCache>,
    new_profile_path: String,
) -> Result<(), String> {
    // 🔥 Cache vollständig leeren
    cache.files.lock().unwrap().clear();

    log!("Profil gewechselt: {} → Cache geleert", new_profile_path);

    Ok(())
}


#[command]
pub fn find_profile_saves(profile_path: String) -> Result<Vec<SaveInfo>, String> {
    let save_root = std::path::Path::new(&profile_path).join("save");

    if !save_root.exists() {
        return Err("Save-Ordner nicht gefunden".into());
    }

    let mut saves = Vec::new();

    let entries = fs::read_dir(&save_root)
        .map_err(|e| format!("Save-Ordner konnte nicht gelesen werden: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
    
        let folder_name = path.file_name().unwrap().to_string_lossy();

        // Nur quicksave, autosave oder numerische Ordner
        if folder_name != "quicksave" && folder_name != "autosave" && folder_name.parse::<u32>().is_err() {
            continue;
        }

        let info_sii = path.join("info.sii");

        let mut save = SaveInfo {
            path: path.display().to_string(),
            folder: path.file_name().unwrap().to_string_lossy().to_string(),
            name: None,
            success: false,
            message: None,
            kind: SaveKind::Invalid, // Default
        };
        
        if info_sii.exists() {
            match decrypt_if_needed(&info_sii) {
                Ok(text) => {
                    save.name = extract_save_name(&text);
                    save.success = save.name.is_some();

                    // 🔹 NEUER FILTER
                    save.kind = if !save.success {
                        SaveKind::Invalid
                    } else if save.folder.to_lowercase() == "autosave"
                        || save.folder.to_lowercase() == "quicksave"
                    {
                        SaveKind::Autosave
                    } else if save.folder.chars().all(|c| c.is_digit(10)) {
                        SaveKind::Manual
                    } else {
                        SaveKind::Invalid
                    };
                }
                Err(e) => {
                    save.message = Some(e);
                    save.kind = SaveKind::Invalid;
                }
            }
        } else {
            save.message = Some("info.sii fehlt".into());
            save.kind = SaveKind::Invalid;
        }
        
        saves.push(save);
    }

    Ok(saves)
}


#[command]
pub fn find_ets2_profiles() -> Vec<ProfileInfo> {
    log!("Starte Profil-Suche…");
    let mut found_profiles = Vec::new();

    if let Some(base) = ets2_base_path() {
        let folders = vec![
            base.join("profiles"),
            base.join("profiles.backup"),
            base.clone(),
        ];

        for folder in folders {
            if !folder.exists() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(folder) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }

                    let sii = path.join("profile.sii");
                    if !sii.exists() {
                        continue;
                    }

                    let text = decrypt_if_needed(&sii).ok();
                    let mut info = ProfileInfo {
                        path: path.display().to_string(),
                        name: None,
                        success: false,
                        message: None,
                    };

                    let from_sii = text.as_ref().and_then(|t| extract_profile_name(t));

                    let from_bak = if from_sii.is_none() {
                        let bak = path.join("profile.bak.sii");
                        if bak.exists() {
                            decrypt_if_needed(&bak)
                                .ok()
                                .and_then(|t| extract_profile_name(&t))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let from_folder = if from_sii.is_none() && from_bak.is_none() {
                        path.file_name()
                            .and_then(|os| os.to_str())
                            .and_then(|s| decode_hex_folder_name(s))
                    } else {
                        None
                    };

                    if let Some(name) = from_sii.or(from_bak).or(from_folder) {
                        info.name = Some(name);
                        info.success = true;
                        info.message = Some("OK".into());
                        log!(
                            "Profil gefunden: {} ({})",
                            info.path,
                            info.name.as_ref().unwrap()
                        );
                    } else {
                        info.message = Some("profile_name nicht gefunden".into());
                        log!("profile_name nicht gefunden in {}", info.path);
                    }

                    found_profiles.push(info);
                }
            }
        }
    }

    log!(
        "Profil-Suche abgeschlossen. Gefunden: {}",
        found_profiles.len()
    );
    found_profiles
}

#[command]
pub fn load_profile(
    profile_path: String,
    profile_state: State<'_, AppProfileState>,
    cache: State<'_, DecryptCache>,
) -> Result<String, String> {
    let autosave = crate::utils::paths::autosave_path(&profile_path);
    if !autosave.exists() {
        return Err(format!("Quicksave nicht gefunden: {}", autosave.display()));
    }

    set_active_profile(profile_path.clone(), profile_state, cache)?;

    log!("Profil erfolgreich geladen: {}", profile_path);
    Ok(format!("Profil geladen: {}", profile_path))
}
