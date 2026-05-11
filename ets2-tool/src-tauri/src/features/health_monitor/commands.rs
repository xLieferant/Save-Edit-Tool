use tauri::{State, command};

use crate::features::logging::service as logging_service;
use crate::shared::current_profile::snapshot_resolved_save_context;
use crate::shared::trace::TraceScope;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::models::{SaveHealthFixResultDto, SaveHealthReportDto};
use super::service;

#[command]
pub async fn get_active_save_health(
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<SaveHealthReportDto, String> {
    let mut trace = TraceScope::new("get_active_save_health");
    let context = logging_service::resolve_active_context(profile_state.inner());
    let selected_game = context
        .selected_game
        .clone()
        .unwrap_or_else(|| "ets2".to_string());
    let resolved = snapshot_resolved_save_context(profile_state.inner())
        .map_err(|error| format!("Failed to resolve active save context: {}", error))?;
    let decrypt_cache = decrypt_cache.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        service::analyze_resolved_save_health(context, resolved, selected_game, &decrypt_cache)
    })
    .await
    .map_err(|error| format!("get_active_save_health join failed: {}", error))?;

    if let Err(error) = result.as_ref() {
        trace.finish_error(error);
        return Err(error.clone());
    }

    trace.finish_ok();
    result
}

#[command]
pub fn apply_save_health_fix(
    fix_id: String,
    confirmed: bool,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<SaveHealthFixResultDto, String> {
    service::apply_safe_fix(
        &fix_id,
        confirmed,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
    )
}
