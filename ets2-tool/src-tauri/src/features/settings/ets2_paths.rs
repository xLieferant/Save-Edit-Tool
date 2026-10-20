use crate::features::health_monitor::service as health_service;
use crate::shared::decrypt::clear_decrypt_cache;
use crate::shared::paths::{
    Ets2DataLocationDto, Ets2PathMode, Ets2PathSettings, ets2_data_location_dto,
    normalize_user_path_input, read_ets2_path_settings, save_ets2_path_settings,
};
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn get_ets2_data_location() -> Result<Ets2DataLocationDto, String> {
    Ok(ets2_data_location_dto())
}

#[tauri::command]
pub fn choose_ets2_data_location(
    app: AppHandle,
    dialog_title: Option<String>,
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
    profile_cache: State<'_, ProfileCache>,
) -> Result<Option<Ets2DataLocationDto>, String> {
    let title = dialog_title
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            crate::features::language::translator::t("modals.ets2_data_location.dialog_title")
        });
    let Some(picked) = app.dialog().file().set_title(title).blocking_pick_folder() else {
        return Ok(None);
    };
    let picked_path = file_path_to_string(picked)?;
    let custom_homedir = normalize_user_path_input(&picked_path)
        .ok_or_else(|| "The selected folder path could not be read.".to_string())?;

    save_custom_location(custom_homedir)?;
    reset_runtime_after_location_change(
        &app,
        profile_state.inner(),
        decrypt_cache.inner(),
        profile_cache.inner(),
    )?;
    Ok(Some(ets2_data_location_dto()))
}

#[tauri::command]
pub fn set_ets2_data_location_mode(
    app: AppHandle,
    mode: String,
    custom_homedir: Option<String>,
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
    profile_cache: State<'_, ProfileCache>,
) -> Result<Ets2DataLocationDto, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "default" => save_ets2_path_settings(&Ets2PathSettings::default())?,
        "custom" => {
            let existing = read_ets2_path_settings();
            let raw_path = custom_homedir
                .filter(|value| !value.trim().is_empty())
                .or(existing.custom_homedir)
                .ok_or_else(|| "Select a custom ETS2 data location first.".to_string())?;
            let custom_path = normalize_user_path_input(&raw_path)
                .ok_or_else(|| "The custom ETS2 data location is empty.".to_string())?;
            save_custom_location(custom_path)?;
        }
        _ => return Err("Invalid ETS2 data location mode.".to_string()),
    }

    reset_runtime_after_location_change(
        &app,
        profile_state.inner(),
        decrypt_cache.inner(),
        profile_cache.inner(),
    )?;
    Ok(ets2_data_location_dto())
}

#[tauri::command]
pub fn reset_ets2_data_location(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
    profile_cache: State<'_, ProfileCache>,
) -> Result<Ets2DataLocationDto, String> {
    save_ets2_path_settings(&Ets2PathSettings::default())?;
    reset_runtime_after_location_change(
        &app,
        profile_state.inner(),
        decrypt_cache.inner(),
        profile_cache.inner(),
    )?;
    Ok(ets2_data_location_dto())
}

fn save_custom_location(custom_homedir: PathBuf) -> Result<(), String> {
    save_ets2_path_settings(&Ets2PathSettings {
        mode: Ets2PathMode::Custom,
        custom_homedir: Some(custom_homedir.display().to_string()),
    })
}

fn reset_runtime_after_location_change(
    app: &AppHandle,
    profile_state: &AppProfileState,
    decrypt_cache: &DecryptCache,
    profile_cache: &ProfileCache,
) -> Result<(), String> {
    {
        let mut current_profile = profile_state
            .current_profile
            .lock()
            .map_err(|_| "current_profile lock poisoned".to_string())?;
        *current_profile = None;
    }
    {
        let mut current_save = profile_state
            .current_save
            .lock()
            .map_err(|_| "current_save lock poisoned".to_string())?;
        *current_save = None;
    }

    clear_decrypt_cache(decrypt_cache)?;
    profile_cache.reset_profile(None);
    health_service::clear_mod_scan_cache();
    clear_profile_cache_files(app)
}

fn clear_profile_cache_files(app: &AppHandle) -> Result<(), String> {
    let Ok(mut dir) = app.path().config_dir() else {
        return Ok(());
    };
    dir.push("save-edit-tool");

    for file_name in ["profiles_cache.json", "last_profile.json"] {
        let path = dir.join(file_name);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Failed to clear cached profile data {}: {}",
                    path.display(),
                    error
                ));
            }
        }
    }

    Ok(())
}

fn file_path_to_string(path: tauri_plugin_dialog::FilePath) -> Result<String, String> {
    path.into_path()
        .map(|value| value.display().to_string())
        .map_err(|_| "The selected dialog path could not be read.".to_string())
}
