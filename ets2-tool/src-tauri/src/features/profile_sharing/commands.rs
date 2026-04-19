use super::models::{
    ProfileShareContext, ProfileShareExportResult, ProfileShareImportPreview,
    ProfileShareImportResult,
};
use super::service;
use crate::state::AppProfileState;
use tauri::AppHandle;
use tauri::State;

#[tauri::command]
pub fn get_profile_share_context(
    profile_path: Option<String>,
    profile_state: State<'_, AppProfileState>,
) -> Result<ProfileShareContext, String> {
    service::get_profile_share_context(profile_path.as_deref(), profile_state.inner())
}

#[tauri::command]
pub async fn pick_shared_profile_import_archive(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
) -> Result<Option<String>, String> {
    service::pick_shared_profile_import_archive(&app, profile_state.inner())
}

#[tauri::command]
pub async fn pick_shared_profile_export_directory(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
) -> Result<Option<String>, String> {
    service::pick_shared_profile_export_directory(&app, profile_state.inner())
}

#[tauri::command]
pub fn export_shared_profile(
    profile_path: String,
    export_dir_override: Option<String>,
    profile_state: State<'_, AppProfileState>,
) -> Result<ProfileShareExportResult, String> {
    service::export_shared_profile(&profile_path, export_dir_override, profile_state.inner())
}

#[tauri::command]
pub fn inspect_shared_profile_archive(
    archive_path: String,
    profile_name_override: Option<String>,
    profile_state: State<'_, AppProfileState>,
) -> Result<ProfileShareImportPreview, String> {
    service::inspect_shared_profile_archive(&archive_path, profile_name_override, profile_state.inner())
}

#[tauri::command]
pub fn import_shared_profile(
    archive_path: String,
    profile_name_override: Option<String>,
    profile_state: State<'_, AppProfileState>,
) -> Result<ProfileShareImportResult, String> {
    service::import_shared_profile(&archive_path, profile_name_override, profile_state.inner())
}
