use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::models::profile_info::ProfileInfo;
use crate::utils::{
    decrypt::decrypt_if_needed,
    extract::extract_profile_name,
    hex::decode_hex_folder_name,
    paths::{autosave_path, resolve_ets2_paths},
};

use tauri::command;

#[command]
pub fn find_ets2_profiles() -> Vec<ProfileInfo> {
    let mut result = Vec::new();

    for folder in resolve_ets2_paths() {
        if !folder.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(folder) {
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

                let content = decrypt_if_needed(&profile_sii).ok();
                let name_from_sii = content
                    .as_ref()
                    .and_then(|txt| extract_profile_name(txt));

                let final_name = name_from_sii.or_else(|| {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .and_then(|s| decode_hex_folder_name(s))
                });

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

#[command]
pub fn load_profile(profile_path: String) -> Result<String, String> {
    let autosave = autosave_path(&profile_path);

    if !autosave.exists() {
        return Err(format!("Autosave nicht gefunden: {}", autosave.display()));
    }

    std::env::set_var("CURRENT_PROFILE", &profile_path);

    Ok(format!("Profil geladen: {}", profile_path))
}
