use crate::log;
use crate::models::profile_info::ProfileInfo;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::extract::extract_profile_name;
use crate::utils::hex::decode_hex_folder_name;
use crate::utils::paths::ets2_base_path;
use std::fs;
use tauri::command;
// use std::path::Path;

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
pub fn load_profile(profile_path: String) -> Result<String, String> {
    let autosave = crate::utils::paths::autosave_path(&profile_path);
    if !autosave.exists() {
        return Err(format!("Quicksave nicht gefunden: {}", autosave.display()));
    }

    std::env::set_var("CURRENT_PROFILE", &profile_path);
    log!("Profil erfolgreich geladen: {}", profile_path);
    Ok(format!("Profil geladen: {}", profile_path))
}
