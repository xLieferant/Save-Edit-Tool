use std::sync::{Mutex, MutexGuard, TryLockError};

use tauri::State;

use crate::features::trailer_change::cache::TrailerChangeSessionCache;
use crate::features::truck_change::cache::TruckChangeSessionCache;
use crate::shared::current_profile::snapshot_active_save_selection;
use crate::state::{AppProfileState, AppState, DecryptCache, ProfileCache};

use super::cache::AiDriverPoolCache;
use super::models::{
    AiDriverPoolSnapshot, DriverAssignmentRequest, DriverAssignmentResult,
    DriverRefAssignmentRequest,
};
use super::service;

fn acquire_mutation_lock(lock: &Mutex<()>) -> Result<MutexGuard<'_, ()>, String> {
    match lock.try_lock() {
        Ok(guard) => Ok(guard),
        Err(TryLockError::WouldBlock) => Err("garage_mutation_in_progress".to_string()),
        Err(TryLockError::Poisoned(_)) => Err("garage_mutation_lock_unavailable".to_string()),
    }
}

#[tauri::command]
pub async fn get_ai_driver_pool(
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
    driver_pool_cache: State<'_, AiDriverPoolCache>,
) -> Result<AiDriverPoolSnapshot, String> {
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "driver_pool_invalid:profile_state_unavailable".to_string())?;
    let decrypt_cache = decrypt_cache.inner().clone();
    let driver_pool_cache = driver_pool_cache.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        service::get_ai_driver_pool(&selection, &decrypt_cache, &driver_pool_cache)
    })
    .await
    .map_err(|error| format!("driver_pool_invalid:task_failed:{error}"))?
}

#[tauri::command]
pub async fn refresh_ai_driver_pool(
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
    driver_pool_cache: State<'_, AiDriverPoolCache>,
) -> Result<AiDriverPoolSnapshot, String> {
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "driver_pool_invalid:profile_state_unavailable".to_string())?;
    let decrypt_cache = decrypt_cache.inner().clone();
    let driver_pool_cache = driver_pool_cache.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        service::refresh_ai_driver_pool(&selection, &decrypt_cache, &driver_pool_cache)
    })
    .await
    .map_err(|error| format!("driver_pool_invalid:task_failed:{error}"))?
}

#[tauri::command]
pub fn assign_random_ai_drivers_to_garage(
    request: DriverAssignmentRequest,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    truck_change_cache: State<'_, TruckChangeSessionCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
    driver_pool_cache: State<'_, AiDriverPoolCache>,
    app_state: State<'_, AppState>,
) -> Result<DriverAssignmentResult, String> {
    let _mutation_guard = acquire_mutation_lock(&app_state.garage_mutation_lock)?;
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "driver_pool_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    service::assign_random_ai_drivers_to_garage(
        &selection,
        &selected_game,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        truck_change_cache.inner(),
        trailer_change_cache.inner(),
        driver_pool_cache.inner(),
        &request,
    )
}
#[tauri::command]
pub fn assign_ai_driver_to_garage(
    request: DriverRefAssignmentRequest,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    truck_change_cache: State<'_, TruckChangeSessionCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
    driver_pool_cache: State<'_, AiDriverPoolCache>,
    app_state: State<'_, AppState>,
) -> Result<DriverAssignmentResult, String> {
    let _mutation_guard = acquire_mutation_lock(&app_state.garage_mutation_lock)?;
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "driver_pool_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    service::assign_ai_driver_to_garage(
        &selection,
        &selected_game,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        truck_change_cache.inner(),
        trailer_change_cache.inner(),
        driver_pool_cache.inner(),
        &request,
    )
}
