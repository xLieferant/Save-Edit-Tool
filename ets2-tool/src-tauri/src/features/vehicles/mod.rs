pub mod editor;
pub mod trailers;
pub mod trucks;

use crate::dev_log;
use crate::shared::decrypt::decrypt_cached_with_cache;
use crate::shared::paths::game_sii_from_save;
use crate::state::{AppProfileState, DecryptCache};
use std::path::Path;

pub(crate) fn load_save_content(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
) -> Result<(String, String), String> {
    let save_path = resolve_active_save_from_snapshot(
        profile_state.current_save.lock().unwrap().clone(),
        profile_state.current_profile.lock().unwrap().clone(),
    )?;

    load_save_content_from_save_path(&save_path, decrypt_cache.inner())
}

pub(crate) fn load_save_content_from_save_path(
    save_path: &str,
    decrypt_cache: &DecryptCache,
) -> Result<(String, String), String> {
    let path = game_sii_from_save(Path::new(&save_path));
    let content = decrypt_cached_with_cache(Path::new(&path), decrypt_cache).map_err(|e| {
        dev_log!("Decrypt Fehler: {}", e);
        e
    })?;

    Ok((content, path.display().to_string()))
}

pub(crate) fn resolve_active_save_from_snapshot(
    current_save: Option<String>,
    current_profile: Option<String>,
) -> Result<String, String> {
    if let Some(save) = current_save {
        return Ok(save);
    }
    let profile = current_profile.ok_or_else(|| "Kein Profil geladen.".to_string())?;
    Ok(format!("{}/save/quicksave", profile))
}
