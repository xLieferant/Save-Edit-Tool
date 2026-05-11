use tauri::{State, command};

use crate::shared::current_profile::snapshot_resolved_save_context;
use crate::shared::trace::TraceScope;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::models::{BackupRestorePreviewDto, BackupRestoreResultDto, BackupVersionDto};
use super::service;

#[command]
pub async fn list_active_save_backups(
    profile_state: State<'_, AppProfileState>,
) -> Result<Vec<BackupVersionDto>, String> {
    let mut trace = TraceScope::new("list_active_save_backups");
    let save_session_id = snapshot_resolved_save_context(profile_state.inner())
        .ok()
        .and_then(|item| item.context.save_session_id);
    let result = tauri::async_runtime::spawn_blocking(move || {
        service::list_backups_for_save_session(save_session_id)
    })
    .await
    .map_err(|error| format!("list_active_save_backups join failed: {}", error))?;

    if let Err(error) = result.as_ref() {
        trace.finish_error(error);
        return Err(error.clone());
    }

    trace.finish_ok();
    result
}

#[command]
pub fn preview_backup_restore(backup_id: String) -> Result<BackupRestorePreviewDto, String> {
    service::build_restore_preview(&backup_id)
}

#[command]
pub fn restore_backup(
    backup_id: String,
    confirmed: bool,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<BackupRestoreResultDto, String> {
    let execution = service::restore_backup(profile_state.inner(), &backup_id, confirmed)?;
    for path in &execution.touched_paths {
        decrypt_cache.invalidate_path(path);
    }
    profile_cache.invalidate_base_config();
    profile_cache.invalidate_save_config();
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();
    Ok(execution.result)
}
