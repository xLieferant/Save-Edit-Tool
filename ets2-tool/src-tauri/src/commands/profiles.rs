use std::fs;

use tauri::command;

use crate::models::profile_info::ProfileInfo;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::extract::extract_profile_name;
use crate::utils::hex::decode_hex_folder_name;
use crate::utils::paths::resolve_ets2_paths;

/// Findet ETS2-Profile in Documents/Euro Truck Simulator 2 (profiles / profiles.backup / root)
#[command]
pub fn find_ets2_profiles() -> Vec<ProfileInfo> {
    let mut result = Vec::new();

    for folder in resolve_ets2_paths() {
        if !folder.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let profile_sii = path.join("profile.sii");
                if !profile_sii.exists() {
                    continue;
                }

                let mut info = ProfileInfo {
                    path: path.display().to_string(),
                    name: None,
                    success: false,
                    message: None,
                };

                // Versuche zu entschlüsseln / lesen
                let content = decrypt_if_needed(&profile_sii).ok();

                // 1) profile.sii
                let from_sii = content.as_ref().and_then(|t| extract_profile_name(t));

                // 2) fallback: profile.bak.sii
                let from_bak = if from_sii.is_none() {
                    let bak = path.join("profile.bak.sii");
                    if bak.exists() {
                        decrypt_if_needed(&bak).ok().and_then(|c| extract_profile_name(&c))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // 3) fallback: decode hex folder name
                let from_hex = if from_sii.is_none() && from_bak.is_none() {
                    path.file_name()
                        .and_then(|os| os.to_str())
                        .and_then(|s| decode_hex_folder_name(s))
                } else {
                    None
                };

                let final_name = from_sii.or(from_bak).or(from_hex);

                if let Some(name) = final_name {
                    info.name = Some(name);
                    info.success = true;
                    info.message = Some("OK".into());
                } else {
                    info.message = Some("profile_name nicht gefunden".into());
                }

                result.push(info);
            }
        }
    }

    result
}

/// Lädt ein Profil (setzt env CURRENT_PROFILE) und prüft, ob Autosave existiert.
#[command]
pub fn load_profile(profile_path: String) -> Result<String, String> {
    let autosave = crate::utils::paths::autosave_path(&profile_path);

    if !autosave.exists() {
        return Err(format!("Autosave nicht gefunden: {}", autosave.display()));
    }

    std::env::set_var("CURRENT_PROFILE", &profile_path);

    Ok(format!("Profil geladen: {}", profile_path))
}
