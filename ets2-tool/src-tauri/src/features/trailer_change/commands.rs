use tauri::{State, command};

use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::cache::TrailerChangeSessionCache;
use super::models::{
    ApplyTrailerChangeResult, TrailerChangePreview, TrailerChangeSession, TrailerSwitchList,
};
use super::service::{
    apply_active_trailer_switch_transaction, read_switch_list, read_switch_preview,
    read_trailer_change_session,
};

#[command]
pub async fn list_owned_trailers_for_switch(
    save_path: Option<String>,
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<TrailerSwitchList, String> {
    read_switch_list(save_path, profile_state.inner(), decrypt_cache.inner())
}

#[command]
pub async fn initialize_trailer_change_session(
    save_path: Option<String>,
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
) -> Result<TrailerChangeSession, String> {
    read_trailer_change_session(
        save_path,
        profile_state.inner(),
        decrypt_cache.inner(),
        trailer_change_cache.inner(),
    )
}

#[command]
pub async fn preview_active_trailer_switch(
    save_path: Option<String>,
    target_trailer_id: String,
    expected_file_hash: String,
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<TrailerChangePreview, String> {
    read_switch_preview(
        save_path,
        target_trailer_id,
        expected_file_hash,
        profile_state.inner(),
        decrypt_cache.inner(),
    )
}

#[command]
pub async fn log_trailer_change_frontend_event(
    event: String,
    detail: Option<String>,
) -> Result<(), String> {
    super::service::log_trailer_change_frontend_event(event, detail)
}

#[command]
pub async fn apply_active_trailer_switch(
    save_path: Option<String>,
    target_trailer_id: String,
    expected_file_hash: String,
    create_persistent_backup: Option<bool>,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
) -> Result<ApplyTrailerChangeResult, String> {
    apply_active_trailer_switch_transaction(
        save_path,
        target_trailer_id,
        expected_file_hash,
        create_persistent_backup.unwrap_or(true),
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        trailer_change_cache.inner(),
    )
}
