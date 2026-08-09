use std::sync::{Mutex, MutexGuard, TryLockError};

use tauri::State;

use crate::features::trailer_change::cache::TrailerChangeSessionCache;
use crate::features::truck_change::cache::TruckChangeSessionCache;
use crate::shared::current_profile::snapshot_active_save_selection;
use crate::state::{AppProfileState, AppState, DecryptCache, ProfileCache};

use super::models::{
    GarageBuyAllRequest, GarageBuyAllResult, GarageListResult, GarageMutationRequest,
    GarageMutationResult, GarageRelinquishEmptyRequest, GarageRelinquishEmptyResult,
    GarageResourceAssignmentRequest, GarageUpdateRequest,
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
pub async fn get_all_garages(
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
    app_state: State<'_, AppState>,
) -> Result<GarageListResult, String> {
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "garage_block_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    let decrypt_cache = decrypt_cache.inner().clone();
    let sqlite_path = app_state.sqlite_path.clone();

    tauri::async_runtime::spawn_blocking(move || {
        service::get_all_garages(&selection, &selected_game, &decrypt_cache, &sqlite_path)
    })
    .await
    .map_err(|error| format!("garage_block_invalid:task_failed:{error}"))?
}

#[tauri::command]
pub fn purchase_garage(
    request: GarageMutationRequest,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    truck_change_cache: State<'_, TruckChangeSessionCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
    app_state: State<'_, AppState>,
) -> Result<GarageMutationResult, String> {
    let _mutation_guard = acquire_mutation_lock(&app_state.garage_mutation_lock)?;
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "garage_block_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    service::purchase_garage(
        &selection,
        &selected_game,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        truck_change_cache.inner(),
        trailer_change_cache.inner(),
        &app_state.sqlite_path,
        &request,
    )
}

#[tauri::command]
pub fn upgrade_owned_garage(
    request: GarageMutationRequest,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    truck_change_cache: State<'_, TruckChangeSessionCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
    app_state: State<'_, AppState>,
) -> Result<GarageMutationResult, String> {
    let _mutation_guard = acquire_mutation_lock(&app_state.garage_mutation_lock)?;
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "garage_block_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    service::upgrade_owned_garage(
        &selection,
        &selected_game,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        truck_change_cache.inner(),
        trailer_change_cache.inner(),
        &app_state.sqlite_path,
        &request,
    )
}

#[tauri::command]
pub fn update_garage(
    request: GarageUpdateRequest,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    truck_change_cache: State<'_, TruckChangeSessionCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
    app_state: State<'_, AppState>,
) -> Result<GarageMutationResult, String> {
    let _mutation_guard = acquire_mutation_lock(&app_state.garage_mutation_lock)?;
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "garage_block_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    service::update_garage(
        &selection,
        &selected_game,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        truck_change_cache.inner(),
        trailer_change_cache.inner(),
        &app_state.sqlite_path,
        &request,
    )
}

#[tauri::command]
pub fn buy_all_garages(
    request: GarageBuyAllRequest,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    truck_change_cache: State<'_, TruckChangeSessionCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
    app_state: State<'_, AppState>,
) -> Result<GarageBuyAllResult, String> {
    let _mutation_guard = acquire_mutation_lock(&app_state.garage_mutation_lock)?;
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "garage_block_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    service::buy_all_garages(
        &selection,
        &selected_game,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        truck_change_cache.inner(),
        trailer_change_cache.inner(),
        &app_state.sqlite_path,
        &request,
    )
}

#[tauri::command]
pub fn relinquish_empty_garages(
    request: GarageRelinquishEmptyRequest,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    truck_change_cache: State<'_, TruckChangeSessionCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
    app_state: State<'_, AppState>,
) -> Result<GarageRelinquishEmptyResult, String> {
    let _mutation_guard = acquire_mutation_lock(&app_state.garage_mutation_lock)?;
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "garage_block_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    service::relinquish_empty_garages(
        &selection,
        &selected_game,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        truck_change_cache.inner(),
        trailer_change_cache.inner(),
        &app_state.sqlite_path,
        &request,
    )
}

#[tauri::command]
pub fn assign_random_garage_resources(
    request: GarageResourceAssignmentRequest,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    truck_change_cache: State<'_, TruckChangeSessionCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
    app_state: State<'_, AppState>,
) -> Result<GarageMutationResult, String> {
    let _mutation_guard = acquire_mutation_lock(&app_state.garage_mutation_lock)?;
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "garage_block_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    service::assign_random_garage_resources(
        &selection,
        &selected_game,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        truck_change_cache.inner(),
        trailer_change_cache.inner(),
        &app_state.sqlite_path,
        &request,
    )
}

#[tauri::command]
pub fn relinquish_garage_ownership(
    request: GarageMutationRequest,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
    truck_change_cache: State<'_, TruckChangeSessionCache>,
    trailer_change_cache: State<'_, TrailerChangeSessionCache>,
    app_state: State<'_, AppState>,
) -> Result<GarageMutationResult, String> {
    let _mutation_guard = acquire_mutation_lock(&app_state.garage_mutation_lock)?;
    let selection = snapshot_active_save_selection(profile_state.inner())
        .map_err(|_| "garage_block_invalid:profile_state_unavailable".to_string())?;
    let selected_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    service::relinquish_garage_ownership(
        &selection,
        &selected_game,
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
        truck_change_cache.inner(),
        trailer_change_cache.inner(),
        &app_state.sqlite_path,
        &request,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::acquire_mutation_lock;

    #[test]
    fn mutation_lock_blocks_overlap_and_is_released_after_drop() {
        let lock = Mutex::new(());
        let guard = acquire_mutation_lock(&lock).unwrap();
        assert_eq!(
            acquire_mutation_lock(&lock).unwrap_err(),
            "garage_mutation_in_progress"
        );

        drop(guard);
        assert!(acquire_mutation_lock(&lock).is_ok());
    }
}
