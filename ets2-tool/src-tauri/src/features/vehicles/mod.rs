pub mod trailers;
pub mod trucks;

use crate::dev_log;
use crate::shared::current_profile::{require_current_profile, require_current_save};
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::paths::game_sii_from_save;
use crate::state::AppProfileState;
use std::path::Path;

pub(crate) fn load_save_content(
    profile_state: tauri::State<'_, AppProfileState>,
) -> Result<String, String> {
    let save_path = require_current_save(profile_state.clone()).or_else(|_| {
        let profile = require_current_profile(profile_state)?;
        Ok::<String, String>(format!("{}/save/quicksave", profile))
    })?;

    let path = game_sii_from_save(Path::new(&save_path));

    decrypt_if_needed(Path::new(&path)).map_err(|e| {
        dev_log!("Decrypt Fehler: {}", e);
        e
    })
}