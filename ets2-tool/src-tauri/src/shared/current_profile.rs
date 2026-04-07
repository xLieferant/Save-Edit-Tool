use std::fs;
use std::path::{Path, PathBuf};

use crate::shared::models::profile::ActiveSaveSelection;
use crate::shared::models::save_context::SaveContext;
use crate::shared::paths::get_base_path;
use crate::state::AppProfileState;
use tauri::State;

#[derive(Debug, Clone)]
pub struct ResolvedSaveContext {
    pub context: SaveContext,
    pub profile_inferred: bool,
    pub save_inferred: bool,
}

pub fn set_current_profile(state: State<'_, AppProfileState>, path: String) {
    *state.current_profile.lock().unwrap() = Some(path);
}

pub fn clear_current_profile(state: State<'_, AppProfileState>) {
    *state.current_profile.lock().unwrap() = None;
}

pub fn get_current_profile(state: State<'_, AppProfileState>) -> Option<String> {
    state.current_profile.lock().unwrap().clone()
}

pub fn require_current_profile(state: State<'_, AppProfileState>) -> Result<String, String> {
    get_current_profile(state).ok_or_else(|| "Kein Profil geladen.".to_string())
}

pub fn require_current_save(state: State<'_, AppProfileState>) -> Result<String, String> {
    state
        .current_save
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "Kein Save geladen.".to_string())
}

pub fn snapshot_active_save_selection(
    state: &AppProfileState,
) -> Result<ActiveSaveSelection, String> {
    let profile_path = state
        .current_profile
        .lock()
        .map_err(|_| "AppProfileState current_profile lock poisoned".to_string())?
        .clone();
    let save_path = state
        .current_save
        .lock()
        .map_err(|_| "AppProfileState current_save lock poisoned".to_string())?
        .clone();

    Ok(ActiveSaveSelection {
        profile_path,
        save_path,
    })
}

pub fn snapshot_resolved_save_context(
    state: &AppProfileState,
) -> Result<ResolvedSaveContext, String> {
    let selection = snapshot_active_save_selection(state)?;
    let selected_game = state
        .selected_game
        .lock()
        .map_err(|_| "AppProfileState selected_game lock poisoned".to_string())?
        .clone();

    let mut profile_path = normalize_existing_path(selection.profile_path);
    let mut save_path = normalize_existing_path(selection.save_path);
    let mut profile_inferred = false;
    let mut save_inferred = false;

    if profile_path.is_none() {
        if let Some(derived_profile) = save_path.as_deref().and_then(derive_profile_from_save_path)
        {
            profile_path = Some(derived_profile);
            profile_inferred = true;
        }
    }

    if let Some(current_profile) = profile_path.clone() {
        if save_path.is_none() {
            if let Some(inferred_save) = latest_save_for_profile(Path::new(&current_profile)) {
                save_path = Some(inferred_save);
                save_inferred = true;
            }
        }
    }

    if profile_path.is_none() || save_path.is_none() {
        let inferred = infer_profile_and_save_from_disk(&selected_game, profile_path.as_deref());
        if profile_path.is_none() {
            profile_path = inferred.0;
            profile_inferred = profile_path.is_some();
        }
        if save_path.is_none() {
            save_path = inferred.1;
            save_inferred = save_path.is_some();
        }
    }

    Ok(ResolvedSaveContext {
        context: SaveContext::from_paths(profile_path, save_path),
        profile_inferred,
        save_inferred,
    })
}

pub fn snapshot_save_context(state: &AppProfileState) -> Result<SaveContext, String> {
    Ok(snapshot_resolved_save_context(state)?.context)
}

fn normalize_existing_path(path: Option<String>) -> Option<String> {
    let value = path?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.replace('\\', "/");
    if Path::new(trimmed).exists() || Path::new(&normalized).exists() {
        Some(normalized)
    } else {
        None
    }
}

fn derive_profile_from_save_path(save_path: &str) -> Option<String> {
    let save_path = Path::new(save_path);
    let save_dir = if save_path.is_file() {
        save_path.parent()?
    } else {
        save_path
    };
    let save_root = save_dir.parent()?;
    if !save_root
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("save"))
        .unwrap_or(false)
    {
        return None;
    }

    let profile_dir = save_root.parent()?;
    if !profile_dir.join("profile.sii").exists() {
        return None;
    }

    Some(profile_dir.display().to_string().replace('\\', "/"))
}

fn infer_profile_and_save_from_disk(
    selected_game: &str,
    preferred_profile: Option<&str>,
) -> (Option<String>, Option<String>) {
    let Some(base_path) = get_base_path(selected_game) else {
        return (None, None);
    };

    if let Some(preferred_profile) = preferred_profile {
        if let Some(save_path) = latest_save_for_profile(Path::new(preferred_profile)) {
            return (Some(preferred_profile.replace('\\', "/")), Some(save_path));
        }
    }

    let mut best_match: Option<(String, String, u128)> = None;
    for profile_dir in collect_profile_directories(&base_path) {
        let Some(save_path) = latest_save_for_profile(&profile_dir) else {
            continue;
        };
        let modified_token = modified_timestamp(Path::new(&save_path)).unwrap_or(0);
        let normalized_profile = profile_dir.display().to_string().replace('\\', "/");
        match best_match.as_ref() {
            Some((_, _, current_modified)) if *current_modified >= modified_token => {}
            _ => {
                best_match = Some((normalized_profile, save_path, modified_token));
            }
        }
    }

    match best_match {
        Some((profile_path, save_path, _)) => (Some(profile_path), Some(save_path)),
        None => (None, None),
    }
}

fn collect_profile_directories(base_path: &Path) -> Vec<PathBuf> {
    let mut profiles = Vec::new();
    for root in [
        base_path.join("profiles"),
        base_path.join("profiles.backup"),
        base_path.to_path_buf(),
    ] {
        if !root.exists() {
            continue;
        }

        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || !path.join("profile.sii").exists() {
                continue;
            }
            profiles.push(path);
        }
    }
    profiles
}

fn latest_save_for_profile(profile_path: &Path) -> Option<String> {
    let save_root = profile_path.join("save");
    if !save_root.exists() {
        return None;
    }

    let mut best_match: Option<(String, u128)> = None;
    let entries = fs::read_dir(save_root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let game_sii = path.join("game.sii");
        if !game_sii.exists() {
            continue;
        }

        let modified_token = modified_timestamp(&game_sii).unwrap_or(0);
        let normalized_path = path.display().to_string().replace('\\', "/");
        match best_match.as_ref() {
            Some((_, current_modified)) if *current_modified >= modified_token => {}
            _ => {
                best_match = Some((normalized_path, modified_token));
            }
        }
    }

    best_match.map(|(path, _)| path)
}

fn modified_timestamp(path: &Path) -> Option<u128> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}
