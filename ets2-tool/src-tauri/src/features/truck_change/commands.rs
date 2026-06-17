use tauri::{State, command};

use crate::shared::ets2data;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::catalog::{load_official_powertrain_catalog, preview_powertrain_change_from_content};
use super::graph::preview_truck_transfer_from_content;
use super::models::{
    ApplyTruckChangeResult, PowertrainCatalog, TruckChangePreview, TruckPowertrainPreview,
    TruckSwitchList, TruckTransferPreview, TruckTransferSelection,
};
use super::service::{
    apply_active_truck_switch_transaction, read_content_for_path, read_switch_list,
    read_switch_preview, resolve_game_sii_path,
};

#[command]
pub async fn list_owned_trucks_for_switch(
    save_path: Option<String>,
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<TruckSwitchList, String> {
    read_switch_list(save_path, profile_state.inner(), decrypt_cache.inner())
}

#[command]
pub async fn preview_active_truck_switch(
    save_path: Option<String>,
    target_truck_id: String,
    expected_file_hash: String,
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<TruckChangePreview, String> {
    read_switch_preview(
        save_path,
        target_truck_id,
        expected_file_hash,
        profile_state.inner(),
        decrypt_cache.inner(),
    )
}

#[command]
pub async fn apply_active_truck_switch(
    save_path: Option<String>,
    target_truck_id: String,
    expected_file_hash: String,
    create_persistent_backup: Option<bool>,
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<ApplyTruckChangeResult, String> {
    apply_active_truck_switch_transaction(
        save_path,
        target_truck_id,
        expected_file_hash,
        create_persistent_backup.unwrap_or(true),
        profile_state.inner(),
        profile_cache.inner(),
        decrypt_cache.inner(),
    )
}

#[command]
pub async fn get_official_powertrain_catalog(
    game: Option<String>,
    game_version: Option<String>,
) -> Result<PowertrainCatalog, String> {
    let game = game.unwrap_or_else(|| "ets2".to_string());
    let game_version = game_version.unwrap_or_else(|| "unknown".to_string());
    load_official_powertrain_catalog(&ets2data::default_repo_root(), &game, &game_version)
}

#[command]
pub async fn preview_truck_powertrain_change(
    save_path: Option<String>,
    truck_id: String,
    engine_data_path: Option<String>,
    transmission_data_path: Option<String>,
    game: Option<String>,
    game_version: Option<String>,
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<TruckPowertrainPreview, String> {
    let game_path = resolve_game_sii_path(save_path, profile_state.inner())?;
    let content =
        crate::shared::decrypt::decrypt_cached_with_cache(&game_path, decrypt_cache.inner())?;
    let catalog = load_official_powertrain_catalog(
        &ets2data::default_repo_root(),
        game.as_deref().unwrap_or("ets2"),
        game_version.as_deref().unwrap_or("unknown"),
    )?;
    Ok(preview_powertrain_change_from_content(
        &content,
        &catalog,
        &truck_id,
        engine_data_path.as_deref(),
        transmission_data_path.as_deref(),
    ))
}

#[command]
pub async fn preview_truck_transfer(
    source_save_path: String,
    target_save_path: String,
    selections: Vec<TruckTransferSelection>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<TruckTransferPreview, String> {
    let (_, source_content) = read_content_for_path(&source_save_path, decrypt_cache.inner())?;
    let (_, target_content) = read_content_for_path(&target_save_path, decrypt_cache.inner())?;
    Ok(preview_truck_transfer_from_content(
        &source_content,
        &target_content,
        &selections,
    ))
}
