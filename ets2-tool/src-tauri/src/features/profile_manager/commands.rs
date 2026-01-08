use crate::dev_log;
use crate::models::cached_profile::CachedProfile;
use crate::models::profile_info::ProfileInfo;
use crate::models::profile_info::SaveKind;
use crate::models::save_info::SaveInfo;
use crate::state::{AppProfileState, DecryptCache};
use crate::shared::current_profile::set_current_profile;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::extract::extract_profile_name;
use crate::shared::extract_save_name::extract_save_name;
use crate::shared::hex_float::decode_hex_folder_name;
use crate::shared::paths::ets2_base_path;
use crate::shared::paths::ats_base_path;
use crate::shared::paths::get_base_path;
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::Manager;
use tauri::State;
use tauri::command;

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
pub fn set_selected_game(
    game: String,
    state: State<'_, AppProfileState>,
) -> Result<String, String> {
    let mut g = state.selected_game.lock().unwrap();
    *g = game.clone();
    dev_log!("Game changed to: {}", game);
    Ok(game)
}

#[command]
pub fn get_selected_game(state: State<'_, AppProfileState>) -> Result<String, String> {
    let g = state.selected_game.lock().unwrap();
    Ok(g.clone())
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

#[command]
pub fn set_active_profile(
    profile_path: String,
    profile_state: State<'_, AppProfileState>,
    cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    // Profil setzen
    *profile_state.current_profile.lock().unwrap() = Some(profile_path.clone());

    // 🔥 Cache vollständig leeren
    cache.files.lock().unwrap().clear();

    dev_log!(
        "Aktives Profil gesetzt & DecryptCache geleert: {}",
        profile_path
    );
    Ok(())
}

#[command]
pub fn set_current_save(
    save_path: String,
    state: State<'_, AppProfileState>,
    cache: State<'_, DecryptCache>,
) -> Result<(), String> {
    *state.current_save.lock().unwrap() = Some(save_path.clone());
    // Cache leeren
    cache.files.lock().unwrap().clear();
    dev_log!("Aktiver Save gesetzt: {}", save_path);
    Ok(())
}

#[tauri::command]
pub fn switch_profile(cache: State<DecryptCache>, new_profile_path: String) -> Result<(), String> {
    // 🔥 Cache vollständig leeren
    cache.files.lock().unwrap().clear();

    dev_log!("Profil gewechselt: {} → Cache geleert", new_profile_path);

    Ok(())
}

#[command]
pub fn find_profile_saves(profile_path: String) -> Result<Vec<SaveInfo>, String> {
    use std::fs;
    use std::path::{Path, PathBuf};

    let save_root = Path::new(&profile_path).join("save");

    if !save_root.is_dir() {
        return Err("Save-Ordner nicht gefunden".into());
    }

    let entries = fs::read_dir(&save_root)
        .map_err(|e| format!("Save-Ordner konnte nicht gelesen werden: {}", e))?;

    let mut saves = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();

        // 🔒 Invariante: Save MUSS ein Ordner sein
        if !path.is_dir() {
            continue;
        }

        let folder = match path.file_name().and_then(|n| n.to_str()) {
            Some(f) => f.to_string(),
            None => continue,
        };

        // 🔹 Nur erlaubte Save-Ordner
        let kind = classify_save_folder(&folder);
        if kind == SaveKind::Invalid {
            continue;
        }

        let info_sii = path.join("info.sii");

        let mut save = SaveInfo {
            path: path.display().to_string(),
            folder: folder.clone(),
            name: None,
            success: false,
            message: None,
            kind,
        };

        // 🔹 info.sii ist optional – aber wichtig für Name/Status
        if info_sii.is_file() {
            match decrypt_if_needed(&info_sii) {
                Ok(text) => {
                    save.name = extract_save_name(&text);
                    save.success = save.name.is_some();

                    if !save.success {
                        save.message = Some("Kein Save-Name gefunden".into());
                        save.kind = SaveKind::Invalid;
                    }
                }
                Err(e) => {
                    save.message = Some(e);
                    save.success = false;
                    save.kind = SaveKind::Invalid;
                }
            }
        } else {
            save.message = Some("info.sii fehlt".into());
            save.success = false;
            save.kind = SaveKind::Invalid;
        }

        saves.push(save);
    }

    Ok(saves)
}

fn classify_save_folder(folder: &str) -> SaveKind {
    match folder.to_lowercase().as_str() {
        "quicksave" | "autosave" => SaveKind::Autosave,
        _ if folder.chars().all(|c| c.is_ascii_digit()) => SaveKind::Manual,
        _ => SaveKind::Invalid,
    }
}

#[command]
pub fn find_ets2_profiles(state: State<'_, AppProfileState>) -> Vec<ProfileInfo> {
    dev_log!("Starte Profil-Suche…");
    let mut found_profiles = Vec::new();

    let game_key = state.selected_game.lock().unwrap().clone();
    let base_opt = get_base_path(&game_key);

    if let Some(base) = base_opt {
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

                    // Check for online avatar
                    let avatar_path = path.join("online_avatar.png");
                    let avatar = if avatar_path.exists() {
                        if let Ok(bytes) = std::fs::read(&avatar_path) {
                            use base64::{Engine as _, engine::general_purpose};
                            let b64 = general_purpose::STANDARD.encode(&bytes);
                            Some(format!("data:image/png;base64,{}", b64))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let text = decrypt_if_needed(&sii).ok();
                    let mut info = ProfileInfo {
                        path: path.display().to_string(),
                        name: None,
                        avatar,
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
                        dev_log!(
                            "Profil gefunden: {} ({})",
                            info.path,
                            info.name.as_ref().unwrap()
                        );
                    } else {
                        info.message = Some("profile_name nicht gefunden".into());
                        dev_log!("profile_name nicht gefunden in {}", info.path);
                    }

                    found_profiles.push(info);
                }
            }
        }
    }

    dev_log!(
        "Profil-Suche abgeschlossen. Gefunden: {}",
        found_profiles.len()
    );
    found_profiles
}

#[command]
pub fn load_profile(
    profile_path: String,
    save_path: Option<String>,
    profile_state: State<'_, AppProfileState>,
    cache: State<'_, DecryptCache>,
) -> Result<String, String> {
    let save_to_load = if let Some(path_str) = save_path {
        PathBuf::from(path_str)
    } else {
        crate::shared::paths::autosave_path(&profile_path)
    };

    if !save_to_load.exists() {
        return Err(format!("Save nicht gefunden: {}", save_to_load.display()));
    }

    // Profil setzen
    set_active_profile(profile_path.clone(), profile_state.clone(), cache.clone())?;

    // 🔥 SAVE SETZEN (Entweder übergeben oder Autosave)
    set_current_save(
        save_to_load.to_string_lossy().to_string(),
        profile_state,
        cache,
    )?;

    dev_log!(
        "Profil geladen: {} | Save: {}",
        profile_path,
        save_to_load.display()
    );
    Ok("Profil geladen".into())
}
