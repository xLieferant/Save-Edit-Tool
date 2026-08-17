use std::fs;
use std::path::{Path, PathBuf};

use crate::features::backup::service as backup_service;
use crate::features::garages::models::{GarageInfo, GarageOwnership, GarageSize};
use crate::features::garages::parser::parse_garages_from_sii;
use crate::features::garages::writer::write_verified_content;
use crate::features::trailer_change::cache::TrailerChangeSessionCache;
use crate::features::truck_change::cache::TruckChangeSessionCache;
use crate::shared::current_profile::snapshot_active_save_selection;
use crate::shared::decrypt::decrypt_cached_with_cache;
use crate::shared::ets2data::validate::sha256_hex_bytes;
use crate::shared::models::profile::ActiveSaveSelection;
use crate::shared::paths::game_sii_from_save;
use crate::shared::user_log;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::cache::AiDriverPoolCache;
use super::models::{
    AiDriverPoolSnapshot, DriverAssignmentRequest, DriverAssignmentResult,
    DriverRefAssignmentRequest,
};
use super::parser::ai_driver_pool_snapshot;
use super::writer::{
    DriverAssignmentPlan, apply_ai_driver_assignment, apply_random_ai_driver_assignment,
    verify_ai_driver_assignment,
};

pub fn get_ai_driver_pool(
    selection: &ActiveSaveSelection,
    decrypt_cache: &DecryptCache,
    driver_pool_cache: &AiDriverPoolCache,
) -> Result<AiDriverPoolSnapshot, String> {
    let game_sii_path = resolve_selected_game_sii(selection)?;
    let profile_id = selection_component_id(selection.profile_path.as_deref());
    let save_id = selection_component_id(selection.save_path.as_deref());
    let game_sii_key = game_sii_path.display().to_string();
    let modified_token = modified_timestamp(&game_sii_path).unwrap_or(0);
    if let Some(snapshot) =
        driver_pool_cache.get(&profile_id, &save_id, &game_sii_key, modified_token)
    {
        return Ok(snapshot);
    }
    refresh_ai_driver_pool(selection, decrypt_cache, driver_pool_cache)
}

pub fn refresh_ai_driver_pool(
    selection: &ActiveSaveSelection,
    decrypt_cache: &DecryptCache,
    driver_pool_cache: &AiDriverPoolCache,
) -> Result<AiDriverPoolSnapshot, String> {
    let game_sii_path = resolve_selected_game_sii(selection)?;
    let profile_id = selection_component_id(selection.profile_path.as_deref());
    let save_id = selection_component_id(selection.save_path.as_deref());
    let content = read_fresh_content(&game_sii_path, decrypt_cache)?;
    let snapshot = build_and_store_snapshot(
        &content,
        profile_id,
        save_id,
        &game_sii_path,
        driver_pool_cache,
    )?;
    let _ = user_log::user_log_info(
        "Drivers",
        format!(
            "AI driver pool refreshed: profile={}, save={}, pool={}, available={}",
            snapshot.profile_id,
            snapshot.save_id,
            snapshot.driver_pool_count,
            snapshot.available_driver_count
        ),
    );
    Ok(snapshot)
}

#[allow(clippy::too_many_arguments)]
pub fn assign_random_ai_drivers_to_garage(
    selection: &ActiveSaveSelection,
    selected_game: &str,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    driver_pool_cache: &AiDriverPoolCache,
    request: &DriverAssignmentRequest,
) -> Result<DriverAssignmentResult, String> {
    if request.count == 0 {
        return Err("garage_assignment_driver_count_invalid:zero".to_string());
    }
    assign_ai_drivers_to_garage_with_plan(
        selection,
        selected_game,
        profile_state,
        profile_cache,
        decrypt_cache,
        truck_change_cache,
        trailer_change_cache,
        driver_pool_cache,
        &request.garage_id,
        &request.expected_save_hash,
        &format!("count={}", request.count),
        |content, garage_id| apply_random_ai_driver_assignment(content, garage_id, request.count),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn assign_ai_driver_to_garage(
    selection: &ActiveSaveSelection,
    selected_game: &str,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    driver_pool_cache: &AiDriverPoolCache,
    request: &DriverRefAssignmentRequest,
) -> Result<DriverAssignmentResult, String> {
    if request.driver_ref.trim().is_empty() {
        return Err("garage_assignment_no_available_driver:driver_ref_invalid".to_string());
    }
    assign_ai_drivers_to_garage_with_plan(
        selection,
        selected_game,
        profile_state,
        profile_cache,
        decrypt_cache,
        truck_change_cache,
        trailer_change_cache,
        driver_pool_cache,
        &request.garage_id,
        &request.expected_save_hash,
        &format!("driver_ref={}", request.driver_ref),
        |content, garage_id| apply_ai_driver_assignment(content, garage_id, &request.driver_ref),
    )
}

#[allow(clippy::too_many_arguments)]
fn assign_ai_drivers_to_garage_with_plan<F>(
    selection: &ActiveSaveSelection,
    selected_game: &str,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    driver_pool_cache: &AiDriverPoolCache,
    garage_id: &str,
    expected_save_hash: &str,
    log_detail: &str,
    build_plan: F,
) -> Result<DriverAssignmentResult, String>
where
    F: FnOnce(&str, &str) -> Result<DriverAssignmentPlan, String>,
{
    if !selected_game.eq_ignore_ascii_case("ets2") {
        return Err(format!(
            "garage_update_not_supported:{}",
            selected_game.to_ascii_lowercase()
        ));
    }
    if garage_id.trim().is_empty() {
        return Err("garage_not_found".to_string());
    }
    if expected_save_hash.trim().is_empty() {
        return Err("save_hash_missing".to_string());
    }

    let game_sii_path = resolve_selected_game_sii(selection)?;
    let profile_id = selection_component_id(selection.profile_path.as_deref());
    let save_id = selection_component_id(selection.save_path.as_deref());
    let content = read_fresh_content(&game_sii_path, decrypt_cache)?;
    let actual_hash = sha256_hex_bytes(content.as_bytes());
    if actual_hash != expected_save_hash {
        return Err("save_changed_since_load".to_string());
    }

    let parsed_garages = parse_garages_from_sii(&content)?;
    let target = parsed_garages
        .garages
        .iter()
        .find(|garage| garage.garage_id == garage_id)
        .ok_or_else(|| format!("garage_not_found:{garage_id}"))?;
    validate_assignment_target(target)?;

    let _ = user_log::user_log_info(
        "Drivers",
        format!(
            "AI driver garage assignment started: profile={profile_id}, save={save_id}, garage={garage_id}, {log_detail}"
        ),
    );
    ensure_active_context(profile_state, selection, selected_game)?;
    let backup = match backup_service::create_backup_for_targets(
        profile_state,
        &format!("garage assign_ai_drivers {garage_id}"),
        &backup_service::recommended_targets(&game_sii_path),
    ) {
        Ok(backup) => backup,
        Err(_) => return Err("backup_failed".to_string()),
    };

    let plan = build_plan(&content, garage_id)?;
    verify_ai_driver_assignment(&content, &plan.content, garage_id, &plan)?;

    ensure_active_context(profile_state, selection, selected_game)?;
    let pre_write_content = read_fresh_content(&game_sii_path, decrypt_cache)?;
    if sha256_hex_bytes(pre_write_content.as_bytes()) != actual_hash {
        return Err("save_changed_since_load".to_string());
    }
    let verify_candidate =
        |candidate: &str| verify_ai_driver_assignment(&content, candidate, garage_id, &plan);
    if let Err(error) = write_verified_content(&game_sii_path, &plan.content, verify_candidate) {
        invalidate_after_write(
            selection,
            &game_sii_path,
            profile_cache,
            decrypt_cache,
            truck_change_cache,
            trailer_change_cache,
            driver_pool_cache,
        );
        return rollback_after_failure(
            profile_state,
            selection,
            &game_sii_path,
            profile_cache,
            decrypt_cache,
            truck_change_cache,
            trailer_change_cache,
            driver_pool_cache,
            &backup.backup_id,
            &actual_hash,
            "save_write_failed",
            error,
            garage_id,
            &profile_id,
            &save_id,
        );
    }

    invalidate_after_write(
        selection,
        &game_sii_path,
        profile_cache,
        decrypt_cache,
        truck_change_cache,
        trailer_change_cache,
        driver_pool_cache,
    );
    let reloaded = match decrypt_cached_with_cache(&game_sii_path, decrypt_cache) {
        Ok(content) => content,
        Err(_) => {
            return rollback_after_failure(
                profile_state,
                selection,
                &game_sii_path,
                profile_cache,
                decrypt_cache,
                truck_change_cache,
                trailer_change_cache,
                driver_pool_cache,
                &backup.backup_id,
                &actual_hash,
                "save_verification_failed",
                "game_sii_not_decrypted".to_string(),
                garage_id,
                &profile_id,
                &save_id,
            );
        }
    };
    if let Err(error) = verify_ai_driver_assignment(&content, &reloaded, garage_id, &plan) {
        return rollback_after_failure(
            profile_state,
            selection,
            &game_sii_path,
            profile_cache,
            decrypt_cache,
            truck_change_cache,
            trailer_change_cache,
            driver_pool_cache,
            &backup.backup_id,
            &actual_hash,
            "save_verification_failed",
            error,
            garage_id,
            &profile_id,
            &save_id,
        );
    }

    let snapshot = build_and_store_snapshot(
        &reloaded,
        profile_id.clone(),
        save_id.clone(),
        &game_sii_path,
        driver_pool_cache,
    )?;
    let _ = user_log::user_log_info(
        "Drivers",
        format!(
            "AI driver garage assignment verified: profile={profile_id}, save={save_id}, garage={garage_id}, assigned={}, remaining_pool={}, backup_id={}",
            plan.assigned_driver_ids.join(","),
            snapshot.driver_pool_count,
            backup.backup_id
        ),
    );

    Ok(result_from_plan(
        garage_id,
        &plan,
        snapshot.driver_pool_count,
        backup.backup_id,
        sha256_hex_bytes(reloaded.as_bytes()),
    ))
}
fn result_from_plan(
    garage_id: &str,
    plan: &DriverAssignmentPlan,
    remaining_pool_size: usize,
    backup_id: String,
    save_hash: String,
) -> DriverAssignmentResult {
    DriverAssignmentResult {
        garage_id: garage_id.to_string(),
        assigned_count: plan.assigned_driver_ids.len(),
        assigned_driver_ids: plan.assigned_driver_ids.clone(),
        remaining_free_slots: plan.remaining_free_slots,
        remaining_pool_size,
        backup_id,
        backup_created: true,
        verified: true,
        save_hash,
        warnings: plan.warnings.clone(),
    }
}

fn validate_assignment_target(garage: &GarageInfo) -> Result<(), String> {
    if !garage.capacity_consistent {
        return Err("garage_capacity_mismatch".to_string());
    }
    if garage.ownership != GarageOwnership::Owned {
        return Err("garage_not_owned".to_string());
    }
    if !matches!(garage.size, GarageSize::Small | GarageSize::Large)
        || !matches!(garage.status, Some(2) | Some(3))
    {
        return Err("garage_state_invalid".to_string());
    }
    if garage.warnings.iter().any(|warning| {
        warning.contains("_reference_unresolved")
            || warning.contains("_reference_ambiguous")
            || warning.contains("_reference_duplicate")
    }) {
        return Err("garage_has_unresolved_references".to_string());
    }
    Ok(())
}

fn read_fresh_content(path: &Path, decrypt_cache: &DecryptCache) -> Result<String, String> {
    decrypt_cache.invalidate_path(path);
    decrypt_cached_with_cache(path, decrypt_cache).map_err(|_| "game_sii_not_decrypted".to_string())
}

fn build_and_store_snapshot(
    content: &str,
    profile_id: String,
    save_id: String,
    game_sii_path: &Path,
    driver_pool_cache: &AiDriverPoolCache,
) -> Result<AiDriverPoolSnapshot, String> {
    let save_hash = sha256_hex_bytes(content.as_bytes());
    let snapshot =
        ai_driver_pool_snapshot(content, profile_id.clone(), save_id.clone(), save_hash)?;
    driver_pool_cache.store(
        profile_id,
        save_id,
        game_sii_path.display().to_string(),
        modified_timestamp(game_sii_path).unwrap_or(0),
        snapshot.clone(),
    );
    Ok(snapshot)
}

fn ensure_active_context(
    profile_state: &AppProfileState,
    expected_selection: &ActiveSaveSelection,
    expected_game: &str,
) -> Result<(), String> {
    let current_selection = snapshot_active_save_selection(profile_state)
        .map_err(|_| "save_changed_since_load:profile_state_unavailable".to_string())?;
    if current_selection != *expected_selection {
        return Err("save_changed_since_load:active_selection_changed".to_string());
    }
    let current_game = profile_state
        .selected_game
        .lock()
        .map_err(|_| "garage_block_invalid:selected_game_unavailable".to_string())?
        .clone();
    if !current_game.eq_ignore_ascii_case(expected_game) {
        return Err("garage_update_not_supported:active_game_changed".to_string());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn rollback_after_failure<T>(
    profile_state: &AppProfileState,
    selection: &ActiveSaveSelection,
    game_sii_path: &Path,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    driver_pool_cache: &AiDriverPoolCache,
    backup_id: &str,
    original_hash: &str,
    failure_code: &str,
    failure_detail: String,
    garage_id: &str,
    profile_id: &str,
    save_id: &str,
) -> Result<T, String> {
    let _ = user_log::user_log_error(
        "Drivers",
        format!(
            "AI driver assignment rollback started: profile={profile_id}, save={save_id}, garage={garage_id}, cause={failure_code}, backup_id={backup_id}"
        ),
    );
    let restore_result = backup_service::restore_backup(profile_state, backup_id, true);
    invalidate_after_write(
        selection,
        game_sii_path,
        profile_cache,
        decrypt_cache,
        truck_change_cache,
        trailer_change_cache,
        driver_pool_cache,
    );
    match restore_result {
        Ok(_) => {
            let restored_content = read_fresh_content(game_sii_path, decrypt_cache)
                .map_err(|_| "rollback_failed:readback".to_string())?;
            if sha256_hex_bytes(restored_content.as_bytes()) != original_hash {
                return Err("rollback_failed:verification_mismatch".to_string());
            }
            Err(format!("{failure_code}:rolled_back:{failure_detail}"))
        }
        Err(_) => Err("rollback_failed:restore_failed".to_string()),
    }
}

fn invalidate_after_write(
    selection: &ActiveSaveSelection,
    game_sii_path: &Path,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    driver_pool_cache: &AiDriverPoolCache,
) {
    decrypt_cache.invalidate_path(game_sii_path);
    profile_cache.invalidate_vehicle_data();
    profile_cache.invalidate_save_data();
    if let Some(profile_id) = selection.profile_path.as_deref() {
        truck_change_cache.invalidate_save(profile_id, game_sii_path);
        trailer_change_cache.invalidate_save(profile_id, game_sii_path);
        if let Some(save_id) = selection.save_path.as_deref() {
            driver_pool_cache.invalidate_save(
                &selection_component_id(Some(profile_id)),
                &selection_component_id(Some(save_id)),
            );
        } else {
            driver_pool_cache.invalidate_profile(&selection_component_id(Some(profile_id)));
        }
    } else {
        driver_pool_cache.invalidate_all();
    }
}

fn resolve_selected_game_sii(selection: &ActiveSaveSelection) -> Result<PathBuf, String> {
    let profile_path = selection
        .profile_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| "profile_not_loaded".to_string())?;
    let save_path = selection
        .save_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| "save_not_loaded".to_string())?;

    let profile_path = PathBuf::from(profile_path);
    if !profile_path.is_dir() {
        return Err("profile_not_loaded:profile_path_invalid".to_string());
    }
    let profile_path = fs::canonicalize(&profile_path)
        .map_err(|_| "profile_not_loaded:profile_path_invalid".to_string())?;
    let profile_save_root = profile_path.join("save");
    if !profile_save_root.is_dir() {
        return Err("save_not_loaded:profile_save_root_missing".to_string());
    }
    let profile_save_root = fs::canonicalize(profile_save_root)
        .map_err(|_| "save_not_loaded:profile_save_root_invalid".to_string())?;

    let selected_path = PathBuf::from(save_path);
    let save_directory = if selected_path.is_file() {
        selected_path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "save_not_loaded:save_path_invalid".to_string())?
    } else {
        selected_path
    };
    if !save_directory.is_dir() {
        return Err("save_not_loaded:save_path_invalid".to_string());
    }
    let save_directory = fs::canonicalize(save_directory)
        .map_err(|_| "save_not_loaded:save_path_invalid".to_string())?;
    if !save_directory.starts_with(&profile_save_root) {
        return Err("save_not_loaded:save_outside_loaded_profile".to_string());
    }

    let game_sii_path = game_sii_from_save(&save_directory);
    if !game_sii_path.is_file() {
        return Err("game_sii_not_found".to_string());
    }
    fs::canonicalize(game_sii_path).map_err(|_| "game_sii_not_found".to_string())
}

fn selection_component_id(path: Option<&str>) -> String {
    let Some(path) = path.filter(|path| !path.trim().is_empty()) else {
        return "<unknown>".to_string();
    };
    let path = Path::new(path);
    let component_path = if path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("game.sii"))
    {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    component_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("<unknown>")
        .replace(['\r', '\n'], "_")
}

fn modified_timestamp(path: &Path) -> Option<u128> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use uuid::Uuid;

    use super::{
        assign_random_ai_drivers_to_garage, get_ai_driver_pool, validate_assignment_target,
    };
    use crate::features::drivers::cache::AiDriverPoolCache;
    use crate::features::drivers::models::DriverAssignmentRequest;
    use crate::features::garages::models::GarageOwnership;
    use crate::features::garages::parser::parse_garages_from_sii;
    use crate::features::trailer_change::cache::TrailerChangeSessionCache;
    use crate::features::truck_change::cache::TruckChangeSessionCache;
    use crate::shared::ets2data::validate::sha256_hex_bytes;
    use crate::shared::models::profile::ActiveSaveSelection;
    use crate::shared::models::save_context::SaveContext;
    use crate::state::{AppProfileState, DecryptCache, ProfileCache};

    const SAMPLE: &str = r#"SiiNunit
{
economy : _economy {
 player: _player
 garages: 2
 garages[0]: garage.berlin
 garages[1]: garage.paris
 driver_pool: 4
 driver_pool[0]: driver.free_a
 driver_pool[1]: driver.free_b
 driver_pool[2]: driver.free_c
 driver_pool[3]: driver.hired_elsewhere
}
player : _player {
 drivers: 1
 drivers[0]: driver.hired_elsewhere
 driver_flags: 1
 driver_flags[0]: 0
 driver_readiness_timer: 1
 driver_readiness_timer[0]: 0
 driver_undrivable_truck_timers: 1
 driver_undrivable_truck_timers[0]: 0
 driver_quit_warned: 0
}
garage : garage.berlin {
 vehicles: 3
 vehicles[0]: null
 vehicles[1]: null
 vehicles[2]: null
 drivers: 3
 drivers[0]: null
 drivers[1]: driver.keep
 drivers[2]: null
 trailers: 0
 status: 2
 profit_log: profit.berlin
 productivity: 0
}
profit_log : profit.berlin {
}
garage : garage.paris {
 vehicles: 3
 vehicles[0]: null
 vehicles[1]: null
 vehicles[2]: null
 drivers: 3
 drivers[0]: driver.hired_elsewhere
 drivers[1]: null
 drivers[2]: null
 trailers: 0
 status: 2
 profit_log: profit.paris
 productivity: 0
}
profit_log : profit.paris {
}
driver_ai : driver.keep {
 assigned_truck: null
}
driver_ai : driver.hired_elsewhere {
 assigned_truck: null
}
driver_ai : driver.free_a {
 assigned_truck: null
}
driver_ai : driver.free_b {
 assigned_truck: null
}
driver_ai : driver.free_c {
 assigned_truck: null
}
}
"#;

    fn create_temp_save(content: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("ets2-driver-service-e2e-{}", Uuid::new_v4()));
        let profile = root.join("profile");
        let save = profile.join("save").join("1");
        fs::create_dir_all(&save).unwrap();
        fs::write(save.join("game.sii"), content).unwrap();
        (root, profile, save)
    }

    fn app_profile_state(profile: &Path, save: &Path) -> AppProfileState {
        let state = AppProfileState::default();
        *state.current_profile.lock().unwrap() = Some(profile.display().to_string());
        *state.current_save.lock().unwrap() = Some(save.display().to_string());
        *state.selected_game.lock().unwrap() = "ets2".to_string();
        state
    }

    fn selection(profile: &Path, save: &Path) -> ActiveSaveSelection {
        ActiveSaveSelection {
            profile_path: Some(profile.display().to_string()),
            save_path: Some(save.display().to_string()),
        }
    }

    fn cleanup_backup_session(profile: &Path, save: &Path) {
        let context = SaveContext::from_paths(
            Some(profile.display().to_string()),
            Some(save.display().to_string()),
        );
        let Some(session_id) = context.save_session_id else {
            return;
        };
        if let Some(parent) = crate::db::sqlite::app_db_path().parent() {
            let _ = fs::remove_dir_all(parent.join("save_backups").join(session_id));
        }
    }

    #[test]
    fn assign_random_ai_drivers_to_garage_e2e_writes_reloads_and_refreshes_pool() {
        let (root, profile, save) = create_temp_save(SAMPLE);
        let game_sii = save.join("game.sii");
        let selection = selection(&profile, &save);
        let profile_state = app_profile_state(&profile, &save);
        let profile_cache = ProfileCache::default();
        let decrypt_cache = DecryptCache::default();
        let truck_change_cache = TruckChangeSessionCache::default();
        let trailer_change_cache = TrailerChangeSessionCache::default();
        let driver_pool_cache = AiDriverPoolCache::default();
        let before_content = fs::read_to_string(&game_sii).unwrap();
        let before_hash = sha256_hex_bytes(before_content.as_bytes());
        let before_garages = parse_garages_from_sii(&before_content).unwrap();
        let before_target = before_garages
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap()
            .clone();
        let before_other = before_garages
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap()
            .clone();

        let result = assign_random_ai_drivers_to_garage(
            &selection,
            "ets2",
            &profile_state,
            &profile_cache,
            &decrypt_cache,
            &truck_change_cache,
            &trailer_change_cache,
            &driver_pool_cache,
            &DriverAssignmentRequest {
                garage_id: "garage.berlin".to_string(),
                expected_save_hash: before_hash,
                count: 1,
            },
        )
        .unwrap();

        assert!(result.verified);
        assert_eq!(result.garage_id, "garage.berlin");
        assert_eq!(result.assigned_count, 1);
        let assigned_driver = result.assigned_driver_ids[0].clone();
        let reloaded = fs::read_to_string(&game_sii).unwrap();
        let after_garages = parse_garages_from_sii(&reloaded).unwrap();
        let after_target = after_garages
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();
        let after_other = after_garages
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap();

        assert_eq!(
            after_target.slots[0].driver_id.as_deref(),
            Some(assigned_driver.as_str())
        );
        assert_eq!(
            after_target.slots[1].driver_id.as_deref(),
            Some("driver.keep")
        );
        assert_eq!(
            after_target.driver_slot_count,
            before_target.driver_slot_count
        );
        assert_eq!(after_target.status, before_target.status);
        assert_eq!(after_target.profit_log_id, before_target.profit_log_id);
        assert_eq!(after_target.productivity, before_target.productivity);
        assert_eq!(after_target.trailer_ids, before_target.trailer_ids);
        assert_eq!(
            after_target
                .slots
                .iter()
                .map(|slot| slot.truck_id.clone())
                .collect::<Vec<_>>(),
            before_target
                .slots
                .iter()
                .map(|slot| slot.truck_id.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(after_other.slots, before_other.slots);
        assert_eq!(after_other.trailer_ids, before_other.trailer_ids);
        assert_eq!(after_other.status, before_other.status);
        assert_eq!(after_other.profit_log_id, before_other.profit_log_id);
        assert_eq!(after_other.productivity, before_other.productivity);

        let assigned = after_garages
            .garages
            .iter()
            .flat_map(|garage| garage.slots.iter())
            .filter_map(|slot| slot.driver_id.as_deref())
            .collect::<Vec<_>>();
        let unique = assigned.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(assigned.len(), unique.len());

        let snapshot = get_ai_driver_pool(&selection, &decrypt_cache, &driver_pool_cache).unwrap();
        assert_eq!(snapshot.save_hash, result.save_hash);
        assert!(
            !snapshot
                .available_driver_ids
                .iter()
                .any(|driver_id| driver_id == &assigned_driver)
        );

        cleanup_backup_session(&profile, &save);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn assignment_target_rejects_not_owned_garage() {
        let parsed = parse_garages_from_sii(SAMPLE).unwrap();
        let mut target = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap()
            .clone();
        target.ownership = GarageOwnership::NotOwned;

        assert_eq!(
            validate_assignment_target(&target).unwrap_err(),
            "garage_not_owned"
        );
    }
}
