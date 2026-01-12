pub mod trailers;
pub mod trucks;
pub mod editor;

use crate::dev_log;
use crate::shared::current_profile::{require_current_profile, require_current_save};
use crate::shared::decrypt::decrypt_cached;
use crate::shared::paths::game_sii_from_save;
use crate::state::{AppProfileState, DecryptCache};
use std::path::Path;

pub(crate) fn load_save_content(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
) -> Result<(String, String), String> {
    let save_path = require_current_save(profile_state.clone()).or_else(|_| {
        let profile = require_current_profile(profile_state)?;
        Ok::<String, String>(format!("{}/save/quicksave", profile))
    })?;

    let path = game_sii_from_save(Path::new(&save_path));

    let content = decrypt_cached(Path::new(&path), &decrypt_cache).map_err(|e| {
        dev_log!("Decrypt Fehler: {}", e);
        e
    })?;

    Ok((content, path.display().to_string()))
}
