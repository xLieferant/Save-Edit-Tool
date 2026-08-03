use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::features::backup::service as backup_service;
use crate::features::trailer_change::cache::TrailerChangeSessionCache;
use crate::features::truck_change::cache::TruckChangeSessionCache;
use crate::shared::current_profile::snapshot_active_save_selection;
use crate::shared::decrypt::decrypt_cached_with_cache;
use crate::shared::ets2data::import;
use crate::shared::ets2data::models::CityQueryFilter;
use crate::shared::ets2data::validate::sha256_hex_bytes;
use crate::shared::models::profile::ActiveSaveSelection;
use crate::shared::paths::game_sii_from_save;
use crate::shared::user_log;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::models::{
    GarageActionResult, GarageInfo, GarageListResult, GarageMutationRequest, GarageMutationResult,
    GarageOperation, GarageOwnership, GarageSize, GarageUpdateRequest,
};
use super::parser::parse_garages_from_sii;
use super::validator::{GarageVerificationSpec, verify_garage_mutation};
use super::writer::{apply_garage_changes, write_verified_content};

#[derive(Debug, Clone)]
struct CityDetails {
    name: String,
    country_code: String,
}

pub fn get_all_garages(
    selection: &ActiveSaveSelection,
    selected_game: &str,
    decrypt_cache: &DecryptCache,
    sqlite_path: &Path,
) -> Result<GarageListResult, String> {
    let result = read_all_garages(selection, selected_game, decrypt_cache, sqlite_path);
    match &result {
        Ok(garage_list) => {
            let _ = user_log::user_log_info(
                "Garages",
                format!(
                    "Garage scan completed: {} garages found.",
                    garage_list.garages.len()
                ),
            );
        }
        Err(error) => {
            let _ = user_log::user_log_error("Garages", format!("Garage scan failed: {error}"));
        }
    }
    result
}

fn read_all_garages(
    selection: &ActiveSaveSelection,
    selected_game: &str,
    decrypt_cache: &DecryptCache,
    sqlite_path: &Path,
) -> Result<GarageListResult, String> {
    let game_sii_path = resolve_selected_game_sii(selection)?;
    decrypt_cache.invalidate_path(&game_sii_path);
    let content = decrypt_cached_with_cache(&game_sii_path, decrypt_cache)
        .map_err(|_| "game_sii_not_decrypted".to_string())?;
    let normalized_content = content.trim_start_matches('﻿').trim_start();
    if !normalized_content.starts_with("SiiNunit") {
        return Err("game_sii_not_decrypted".to_string());
    }

    let mut parsed = parse_garages_from_sii(&content)?;
    if selected_game.eq_ignore_ascii_case("ets2") {
        if enrich_city_data(&mut parsed.garages, sqlite_path).is_err() {
            parsed
                .diagnostics
                .warnings
                .push("garage_city_dataset_unavailable:ets2".to_string());
            let _ = user_log::user_log_warn("Garages", "Garage city lookup unavailable for ETS2.");
        }
    } else {
        parsed
            .diagnostics
            .warnings
            .push(format!("garage_city_dataset_unavailable:{selected_game}"));
    }

    Ok(GarageListResult {
        game: selected_game.to_ascii_lowercase(),
        save_hash: sha256_hex_bytes(content.as_bytes()),
        headquarters_garage_id: parsed.headquarters_garage_id,
        garages: parsed.garages,
        diagnostics: parsed.diagnostics,
    })
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

fn enrich_city_data(
    garages: &mut [super::models::GarageInfo],
    sqlite_path: &Path,
) -> Result<(), String> {
    let connection = Connection::open(sqlite_path).map_err(|error| error.to_string())?;
    let records = import::list_cities(&connection, Some(CityQueryFilter::default()))?;
    let mut city_by_token: HashMap<String, Option<CityDetails>> = HashMap::new();
    for record in records {
        let token = record.game_token.to_ascii_lowercase();
        let name = if record.name_local.trim().is_empty() {
            record.name_en
        } else {
            record.name_local
        };
        let details = CityDetails {
            name,
            country_code: record.country_iso2,
        };
        city_by_token
            .entry(token)
            .and_modify(|existing| *existing = None)
            .or_insert(Some(details));
    }

    for garage in garages {
        let Some(city_token) = garage.city_token.as_deref() else {
            continue;
        };
        match city_by_token.get(&city_token.to_ascii_lowercase()) {
            Some(Some(city)) => {
                garage.city_name = Some(city.name.clone());
                garage.country_code = Some(city.country_code.clone());
            }
            Some(None) => garage.warnings.push(format!(
                "garage_city_reference_ambiguous:{}",
                garage.garage_id
            )),
            None => garage
                .warnings
                .push(format!("garage_city_not_found:{}", garage.garage_id)),
        }
    }
    Ok(())
}

pub fn purchase_garage(
    selection: &ActiveSaveSelection,
    selected_game: &str,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    sqlite_path: &Path,
    request: &GarageMutationRequest,
) -> Result<GarageMutationResult, String> {
    mutate_garage(
        selection,
        selected_game,
        profile_state,
        profile_cache,
        decrypt_cache,
        truck_change_cache,
        trailer_change_cache,
        sqlite_path,
        &request.garage_id,
        &request.expected_save_hash,
        GarageOperation::Purchase,
        Some(GarageSize::Large),
        false,
    )
}

pub fn buy_all_garages (
    selection: &ActiveSaveSelection,
    selected_game: &str,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    sqlite_path: &Path,
    request: &GarageBuyAllRequest,
) -> Result<GarageMutationResult, String> {
    // Implementation for buying all garages
    mutate_garage(
        selection,
        selected_game,
        profile_state,
        profile_cache,
        decrypt_cache,
        truck_change_cache,
        trailer_change_cache,
        sqlite_path,
        &request.garage_id,
        &request.expected_save_hash,
        GarageOperation::PurchaseAll, // Assuming a new operation type for buying all garages
        None, // No specific target size for buying all garages
        false, // Not setting as headquarters
    )
}

pub fn upgrade_owned_garage(
    selection: &ActiveSaveSelection,
    selected_game: &str,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    sqlite_path: &Path,
    request: &GarageMutationRequest,
) -> Result<GarageMutationResult, String> {
    mutate_garage(
        selection,
        selected_game,
        profile_state,
        profile_cache,
        decrypt_cache,
        truck_change_cache,
        trailer_change_cache,
        sqlite_path,
        &request.garage_id,
        &request.expected_save_hash,
        GarageOperation::Upgrade,
        Some(GarageSize::Large),
        false,
    )
}

pub fn update_garage(
    selection: &ActiveSaveSelection,
    selected_game: &str,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    sqlite_path: &Path,
    request: &GarageUpdateRequest,
) -> Result<GarageMutationResult, String> {
    mutate_garage(
        selection,
        selected_game,
        profile_state,
        profile_cache,
        decrypt_cache,
        truck_change_cache,
        trailer_change_cache,
        sqlite_path,
        &request.garage_id,
        &request.expected_save_hash,
        GarageOperation::Update,
        request.target_size,
        request.set_as_headquarters,
    )
}

#[allow(clippy::too_many_arguments)]
fn mutate_garage(
    selection: &ActiveSaveSelection,
    selected_game: &str,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    sqlite_path: &Path,
    garage_id: &str,
    expected_save_hash: &str,
    operation: GarageOperation,
    target_size: Option<GarageSize>,
    set_as_headquarters: bool,
) -> Result<GarageMutationResult, String> {
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
    let action = mutation_action_label(&operation, target_size, set_as_headquarters);
    let profile_id = selection_component_id(selection.profile_path.as_deref());
    let save_id = selection_component_id(selection.save_path.as_deref());
    let content = read_fresh_content(&game_sii_path, decrypt_cache)?;
    let actual_hash = sha256_hex_bytes(content.as_bytes());
    if actual_hash != expected_save_hash {
        return Err("save_changed_since_load".to_string());
    }

    let parsed = parse_garages_from_sii(&content)?;
    let current = parsed
        .garages
        .iter()
        .find(|garage| garage.garage_id == garage_id)
        .ok_or_else(|| format!("garage_not_found:{garage_id}"))?;
    validate_mutation_target(current, &operation, target_size)?;
    let target_status_and_capacity =
        mutation_target(current, &operation, target_size, set_as_headquarters)?;
    let verification_spec = GarageVerificationSpec {
        operation: operation.clone(),
        target_size,
        set_as_headquarters,
    };

    let _ = user_log::user_log_info(
        "Garages",
        format!(
            "Garage mutation started: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, before={}, expected={}",
            garage_state_summary(current),
            expected_state_summary(target_size, set_as_headquarters)
        ),
    );
    ensure_active_context(profile_state, selection, selected_game)?;
    let backup = match backup_service::create_backup_for_targets(
        profile_state,
        &format!("garage {action} {garage_id}"),
        &backup_service::recommended_targets(&game_sii_path),
    ) {
        Ok(backup) => backup,
        Err(_) => {
            let _ = user_log::user_log_error(
                "Garages",
                format!(
                    "Garage backup failed: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}"
                ),
            );
            return Err("backup_failed".to_string());
        }
    };
    let _ = user_log::user_log_info(
        "Garages",
        format!(
            "Garage backup created: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, backup_id={}",
            backup.backup_id
        ),
    );

    let plan = apply_garage_changes(
        &content,
        garage_id,
        target_status_and_capacity,
        set_as_headquarters,
    )?;
    let predicted = verify_garage_mutation(&content, &plan.content, garage_id, &verification_spec)?;
    let _ = user_log::user_log_info(
        "Garages",
        format!(
            "Garage mutation validated in memory: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, expected_after={}",
            garage_state_summary(&predicted.updated_state)
        ),
    );

    ensure_active_context(profile_state, selection, selected_game)?;
    let pre_write_content = read_fresh_content(&game_sii_path, decrypt_cache)?;
    if sha256_hex_bytes(pre_write_content.as_bytes()) != actual_hash {
        return Err("save_changed_since_load".to_string());
    }
    let verify_candidate = |candidate: &str| {
        verify_garage_mutation(&content, candidate, garage_id, &verification_spec).map(|_| ())
    };
    if let Err(error) = write_verified_content(&game_sii_path, &plan.content, verify_candidate) {
        let _ = user_log::user_log_error(
            "Garages",
            format!(
                "Garage writer failed: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, result={error}"
            ),
        );
        invalidate_after_write(
            selection,
            &game_sii_path,
            profile_cache,
            decrypt_cache,
            truck_change_cache,
            trailer_change_cache,
        );
        let target_changed = decrypt_cached_with_cache(&game_sii_path, decrypt_cache)
            .map(|current| sha256_hex_bytes(current.as_bytes()) != actual_hash)
            .unwrap_or(true);
        if target_changed {
            return rollback_after_failure(
                profile_state,
                selection,
                &game_sii_path,
                profile_cache,
                decrypt_cache,
                truck_change_cache,
                trailer_change_cache,
                &backup.backup_id,
                &actual_hash,
                "save_write_failed",
                error,
                garage_id,
                action,
                &profile_id,
                &save_id,
            );
        }
        return Err(error);
    }
    let _ = user_log::user_log_info(
        "Garages",
        format!(
            "Garage writer completed: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}"
        ),
    );

    invalidate_after_write(
        selection,
        &game_sii_path,
        profile_cache,
        decrypt_cache,
        truck_change_cache,
        trailer_change_cache,
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
                &backup.backup_id,
                &actual_hash,
                "save_verification_failed",
                "game_sii_not_decrypted".to_string(),
                garage_id,
                action,
                &profile_id,
                &save_id,
            );
        }
    };
    let verified_after =
        match verify_garage_mutation(&content, &reloaded, garage_id, &verification_spec) {
            Ok(verified) => verified,
            Err(error) => {
                return rollback_after_failure(
                    profile_state,
                    selection,
                    &game_sii_path,
                    profile_cache,
                    decrypt_cache,
                    truck_change_cache,
                    trailer_change_cache,
                    &backup.backup_id,
                    &actual_hash,
                    "save_verification_failed",
                    error,
                    garage_id,
                    action,
                    &profile_id,
                    &save_id,
                );
            }
        };

    let mut previous_state = verified_after.previous_state;
    let mut updated_state = verified_after.updated_state;
    if enrich_city_data(std::slice::from_mut(&mut previous_state), sqlite_path).is_err()
        || enrich_city_data(std::slice::from_mut(&mut updated_state), sqlite_path).is_err()
    {
        previous_state
            .warnings
            .push("garage_city_dataset_unavailable:ets2".to_string());
        updated_state
            .warnings
            .push("garage_city_dataset_unavailable:ets2".to_string());
    }

    let mut warnings = Vec::new();
    if matches!(
        operation,
        GarageOperation::Purchase | GarageOperation::Upgrade
    ) {
        warnings.push(format!(
            "garage_{}_without_financial_transaction",
            match operation {
                GarageOperation::Purchase => "purchase",
                GarageOperation::Upgrade => "upgrade",
                GarageOperation::Update => unreachable!(),
            }
        ));
    }
    let _ = user_log::user_log_info(
        "Garages",
        format!(
            "Garage mutation verified: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, actual_after={}, backup_id={}, writer=ok, verification=ok",
            garage_state_summary(&updated_state),
            backup.backup_id
        ),
    );

    Ok(GarageMutationResult {
        garage_id: garage_id.to_string(),
        operation,
        previous_state,
        updated_state,
        backup_id: backup.backup_id,
        backup_created: true,
        verified: true,
        financial_transaction_applied: false,
        save_hash: sha256_hex_bytes(reloaded.as_bytes()),
        warnings,
    })
}

fn read_fresh_content(path: &Path, decrypt_cache: &DecryptCache) -> Result<String, String> {
    decrypt_cache.invalidate_path(path);
    decrypt_cached_with_cache(path, decrypt_cache).map_err(|_| "game_sii_not_decrypted".to_string())
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

fn mutation_action_label(
    operation: &GarageOperation,
    target_size: Option<GarageSize>,
    set_as_headquarters: bool,
) -> &'static str {
    match (operation, target_size, set_as_headquarters) {
        (GarageOperation::Purchase, _, _) => "purchase",
        (GarageOperation::Upgrade, _, _) => "upgrade",
        (GarageOperation::Update, Some(GarageSize::Small), false) => "downgrade",
        (GarageOperation::Update, Some(GarageSize::Large), false) => "upgrade",
        (GarageOperation::Update, None, true) => "headquarters",
        (GarageOperation::Update, Some(_), true) => "size_and_headquarters",
        (GarageOperation::Update, _, false) => "update",
    }
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

fn garage_state_summary(garage: &GarageInfo) -> String {
    format!(
        "ownership={:?},size={:?},status={:?},vehicle_slots={},driver_slots={},trailer_slots={},trucks={},drivers={},trailers={},hq={},warnings={}",
        garage.ownership,
        garage.size,
        garage.status,
        garage.vehicle_slot_count,
        garage.driver_slot_count,
        garage.trailer_slot_count,
        garage.assigned_truck_count,
        garage.assigned_driver_count,
        garage.assigned_trailer_count,
        garage.is_headquarters,
        garage.warnings.len()
    )
}

fn expected_state_summary(target_size: Option<GarageSize>, set_as_headquarters: bool) -> String {
    format!(
        "target_size={target_size:?},set_as_headquarters={set_as_headquarters},references=preserved,other_garages=unchanged"
    )
}

fn verify_restored_content(
    original_hash: &str,
    restored_content: &str,
) -> Result<super::parser::ParsedGarageList, String> {
    if sha256_hex_bytes(restored_content.as_bytes()) != original_hash {
        return Err("rollback_failed:verification_mismatch".to_string());
    }
    parse_garages_from_sii(restored_content)
        .map_err(|_| "rollback_failed:invalid_restored_save".to_string())
}

fn validate_mutation_target(
    garage: &GarageInfo,
    operation: &GarageOperation,
    target_size: Option<GarageSize>,
) -> Result<(), String> {
    if !garage.capacity_consistent {
        return Err("garage_capacity_mismatch".to_string());
    }
    if garage
        .warnings
        .iter()
        .any(|warning| warning.starts_with("garage_headquarters_not_owned"))
    {
        return Err("garage_state_invalid".to_string());
    }
    if garage.warnings.iter().any(|warning| {
        warning.contains("_reference_unresolved")
            || warning.contains("_reference_ambiguous")
            || warning.contains("_reference_duplicate")
            || warning.starts_with("garage_slot_assignment_inconsistent")
    }) {
        if matches!(operation, GarageOperation::Update)
            && target_size == Some(GarageSize::Small)
            && garage.size == GarageSize::Large
        {
            return Err("garage_downgrade_has_unresolved_references".to_string());
        }
        return Err("garage_has_unresolved_references".to_string());
    }
    Ok(())
}

fn mutation_target(
    garage: &GarageInfo,
    operation: &GarageOperation,
    target_size: Option<GarageSize>,
    set_as_headquarters: bool,
) -> Result<Option<(i32, usize)>, String> {
    match operation {
        GarageOperation::Purchase => {
            if target_size != Some(GarageSize::Large) {
                return Err("garage_size_invalid".to_string());
            }
            if garage.ownership == GarageOwnership::Owned {
                return Err("garage_already_owned".to_string());
            }
            if garage.ownership != GarageOwnership::NotOwned
                || garage.size != GarageSize::Unowned
                || garage.status != Some(0)
                || garage.vehicle_slot_count != 0
                || garage.driver_slot_count != 0
                || garage.trailer_slot_count != 0
            {
                return Err("garage_state_invalid".to_string());
            }
            Ok(Some((3, 5)))
        }
        GarageOperation::Upgrade => {
            if garage.ownership != GarageOwnership::Owned {
                return Err("garage_not_owned".to_string());
            }
            if garage.size == GarageSize::Large {
                return Err("garage_already_maximum_size".to_string());
            }
            if garage.size != GarageSize::Small
                || garage.status != Some(2)
                || garage.vehicle_slot_count != 3
                || garage.driver_slot_count != 3
            {
                return Err("garage_state_invalid".to_string());
            }
            Ok(Some((3, 5)))
        }
        GarageOperation::Update => {
            if garage.ownership != GarageOwnership::Owned {
                return Err("garage_not_owned".to_string());
            }
            let size_change = match target_size {
                Some(GarageSize::Large) if garage.size == GarageSize::Small => Some((3, 5)),
                Some(GarageSize::Large) if garage.size == GarageSize::Large => None,
                Some(GarageSize::Small) if garage.size == GarageSize::Small => None,
                Some(GarageSize::Small) if garage.size == GarageSize::Large => {
                    validate_downgrade_capacity(garage)?;
                    Some((2, 3))
                }
                Some(_) => return Err("garage_size_invalid".to_string()),
                None => None,
            };
            if size_change.is_none() {
                if set_as_headquarters && !garage.is_headquarters {
                    return Ok(None);
                }
                if target_size.is_some() {
                    return Err("garage_size_already_selected".to_string());
                }
                return Err("garage_update_empty".to_string());
            }
            Ok(size_change)
        }
    }
}

fn validate_downgrade_capacity(garage: &GarageInfo) -> Result<(), String> {
    // A 5-to-3 downgrade is safe only when slots 3 and 4 contain neither a
    // truck nor a driver. Trailer references are a separate variable-length
    // garage list in ETS2 and are preserved without resizing or relocation.
    let out_of_range_slots = garage
        .slots
        .iter()
        .filter(|slot| slot.index >= 3 && (slot.truck_id.is_some() || slot.driver_id.is_some()))
        .map(|slot| slot.index.to_string())
        .collect::<Vec<_>>();
    if garage.occupied_slots > 3 || !out_of_range_slots.is_empty() {
        return Err(format!(
            "garage_downgrade_capacity_exceeded:occupied={}:target=3:slots={}",
            garage.occupied_slots,
            out_of_range_slots.join(",")
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn rollback_after_failure(
    profile_state: &AppProfileState,
    selection: &ActiveSaveSelection,
    game_sii_path: &Path,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
    backup_id: &str,
    original_hash: &str,
    failure_code: &str,
    failure_detail: String,
    garage_id: &str,
    action: &str,
    profile_id: &str,
    save_id: &str,
) -> Result<GarageMutationResult, String> {
    let _ = user_log::user_log_error(
        "Garages",
        format!(
            "Garage rollback started: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, cause={failure_code}, backup_id={backup_id}"
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
    );
    match restore_result {
        Ok(_) => {
            let restored_content = match read_fresh_content(game_sii_path, decrypt_cache) {
                Ok(content) => content,
                Err(_) => {
                    let _ = user_log::user_log_error(
                        "Garages",
                        format!(
                            "Garage rollback verification failed: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, result=read_failed"
                        ),
                    );
                    return Err("rollback_failed:readback".to_string());
                }
            };
            let restored = match verify_restored_content(original_hash, &restored_content) {
                Ok(parsed) => parsed,
                Err(error) => {
                    let _ = user_log::user_log_error(
                        "Garages",
                        format!(
                            "Garage rollback verification failed: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, result={error}"
                        ),
                    );
                    return Err(error);
                }
            };
            let restored_state = restored
                .garages
                .iter()
                .find(|garage| garage.garage_id == garage_id)
                .map(garage_state_summary)
                .unwrap_or_else(|| "garage=missing".to_string());
            let _ = user_log::user_log_error(
                "Garages",
                format!(
                    "Garage rollback verified: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, result=restored, actual={restored_state}"
                ),
            );
            Err(format!("{failure_code}:rolled_back:{failure_detail}"))
        }
        Err(_) => {
            let _ = user_log::user_log_error(
                "Garages",
                format!(
                    "Garage rollback failed: profile={profile_id}, save={save_id}, garage={garage_id}, action={action}, result=restore_failed"
                ),
            );
            Err("rollback_failed:restore_failed".to_string())
        }
    }
}

fn invalidate_after_write(
    selection: &ActiveSaveSelection,
    game_sii_path: &Path,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
    trailer_change_cache: &TrailerChangeSessionCache,
) {
    decrypt_cache.invalidate_path(game_sii_path);
    profile_cache.invalidate_vehicle_data();
    profile_cache.invalidate_save_data();
    if let Some(profile_id) = selection.profile_path.as_deref() {
        truck_change_cache.invalidate_save(profile_id, game_sii_path);
        trailer_change_cache.invalidate_save(profile_id, game_sii_path);
    }
}

fn placeholder(action: &str) -> GarageActionResult {
    GarageActionResult {
        action: action.to_string(),
        implemented: false,
    }
}

pub fn buy_garage() -> GarageActionResult {
    placeholder("buy_garage")
}

pub fn upgrade_garage() -> GarageActionResult {
    placeholder("upgrade_garage")
}

pub fn relinquish_garage_ownership() -> GarageActionResult {
    placeholder("relinquish_garage_ownership")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{
        ensure_active_context, mutation_target, resolve_selected_game_sii,
        validate_mutation_target, verify_restored_content,
    };
    use crate::features::garages::models::{GarageMutationRequest, GarageOperation, GarageSize};
    use crate::features::garages::parser::parse_garages_from_sii;
    use crate::features::trailer_change::cache::TrailerChangeSessionCache;
    use crate::features::truck_change::cache::TruckChangeSessionCache;
    use crate::shared::ets2data::validate::sha256_hex_bytes;
    use crate::shared::models::profile::ActiveSaveSelection;
    use crate::state::{AppProfileState, DecryptCache, ProfileCache};
    use uuid::Uuid;

    const SAMPLE: &str = include_str!("../../../test-fixtures/garages/garage_samples.sii");

    fn garage(garage_id: &str) -> crate::features::garages::models::GarageInfo {
        parse_garages_from_sii(SAMPLE)
            .unwrap()
            .garages
            .into_iter()
            .find(|garage| garage.garage_id == garage_id)
            .unwrap()
    }

    #[test]
    fn active_save_requires_loaded_profile() {
        let error = resolve_selected_game_sii(&ActiveSaveSelection::default()).unwrap_err();
        assert_eq!(error, "profile_not_loaded");
    }

    #[test]
    fn active_save_requires_loaded_save() {
        let selection = ActiveSaveSelection {
            profile_path: Some("not-used-without-save".to_string()),
            save_path: None,
        };
        let error = resolve_selected_game_sii(&selection).unwrap_err();
        assert_eq!(error, "save_not_loaded");
    }

    #[test]
    fn active_save_rejects_save_outside_loaded_profile() {
        let root =
            std::env::temp_dir().join(format!("ets2-garage-selection-test-{}", Uuid::new_v4()));
        let profile = root.join("profile");
        let profile_save_root = profile.join("save");
        let outside_save = root.join("outside-save");
        fs::create_dir_all(&profile_save_root).unwrap();
        fs::create_dir_all(&outside_save).unwrap();
        fs::write(outside_save.join("game.sii"), SAMPLE).unwrap();
        let selection = ActiveSaveSelection {
            profile_path: Some(profile.display().to_string()),
            save_path: Some(outside_save.display().to_string()),
        };

        let error = resolve_selected_game_sii(&selection).unwrap_err();

        assert_eq!(error, "save_not_loaded:save_outside_loaded_profile");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mutation_rejects_ats_before_reading_save() {
        let error = super::purchase_garage(
            &ActiveSaveSelection::default(),
            "ats",
            &AppProfileState::default(),
            &ProfileCache::default(),
            &DecryptCache::default(),
            &TruckChangeSessionCache::default(),
            &TrailerChangeSessionCache::default(),
            Path::new("unused"),
            &GarageMutationRequest {
                garage_id: "garage.los_angeles".to_string(),
                expected_save_hash: "unused".to_string(),
            },
        )
        .unwrap_err();

        assert_eq!(error, "garage_update_not_supported:ats");
    }

    #[test]
    fn mutation_rejects_save_changed_since_load() {
        let root = std::env::temp_dir().join(format!("ets2-garage-hash-test-{}", Uuid::new_v4()));
        let profile = root.join("profile");
        let save = profile.join("save").join("1");
        fs::create_dir_all(&save).unwrap();
        fs::write(save.join("game.sii"), SAMPLE).unwrap();
        let selection = ActiveSaveSelection {
            profile_path: Some(profile.display().to_string()),
            save_path: Some(save.display().to_string()),
        };

        let error = super::purchase_garage(
            &selection,
            "ets2",
            &AppProfileState::default(),
            &ProfileCache::default(),
            &DecryptCache::default(),
            &TruckChangeSessionCache::default(),
            &TrailerChangeSessionCache::default(),
            Path::new("unused"),
            &GarageMutationRequest {
                garage_id: "garage.los_angeles".to_string(),
                expected_save_hash: "outdated".to_string(),
            },
        )
        .unwrap_err();

        assert_eq!(error, "save_changed_since_load");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn purchase_requires_unowned_garage() {
        let error = mutation_target(
            &garage("garage.berlin"),
            &GarageOperation::Purchase,
            Some(GarageSize::Large),
            false,
        )
        .unwrap_err();
        assert_eq!(error, "garage_already_owned");

        let target = mutation_target(
            &garage("garage.los_angeles"),
            &GarageOperation::Purchase,
            Some(GarageSize::Large),
            false,
        )
        .unwrap();
        assert_eq!(target, Some((3, 5)));

        let small_purchase_error = mutation_target(
            &garage("garage.los_angeles"),
            &GarageOperation::Purchase,
            Some(GarageSize::Small),
            false,
        )
        .unwrap_err();
        assert_eq!(small_purchase_error, "garage_size_invalid");

        let mut inconsistent = garage("garage.los_angeles");
        inconsistent.trailer_slot_count = 1;
        let trailer_error = mutation_target(
            &inconsistent,
            &GarageOperation::Purchase,
            Some(GarageSize::Large),
            false,
        )
        .unwrap_err();
        assert_eq!(trailer_error, "garage_state_invalid");
    }

    #[test]
    fn upgrade_requires_small_owned_garage() {
        let not_owned_error = mutation_target(
            &garage("garage.los_angeles"),
            &GarageOperation::Upgrade,
            Some(GarageSize::Large),
            false,
        )
        .unwrap_err();
        assert_eq!(not_owned_error, "garage_not_owned");

        let maximum_error = mutation_target(
            &garage("garage.berlin"),
            &GarageOperation::Upgrade,
            Some(GarageSize::Large),
            false,
        )
        .unwrap_err();
        assert_eq!(maximum_error, "garage_already_maximum_size");
    }

    #[test]
    fn update_allows_safe_downsize_and_rejects_occupied_removed_slots() {
        let target = mutation_target(
            &garage("garage.berlin"),
            &GarageOperation::Update,
            Some(GarageSize::Small),
            false,
        )
        .unwrap();
        assert_eq!(target, Some((2, 3)));

        let mut occupied = garage("garage.berlin");
        occupied.slots[4].truck_id = Some("truck.five".to_string());
        let downsize_error = mutation_target(
            &occupied,
            &GarageOperation::Update,
            Some(GarageSize::Small),
            false,
        )
        .unwrap_err();
        assert!(downsize_error.starts_with("garage_downgrade_capacity_exceeded"));
    }

    #[test]
    fn update_rejects_already_selected_size() {
        let error = mutation_target(
            &garage("garage.paris"),
            &GarageOperation::Update,
            Some(GarageSize::Small),
            false,
        )
        .unwrap_err();
        assert_eq!(error, "garage_size_already_selected");
    }

    #[test]
    fn update_allows_headquarters_change_without_size_change() {
        let target = mutation_target(
            &garage("garage.paris"),
            &GarageOperation::Update,
            None,
            true,
        )
        .unwrap();
        assert_eq!(target, None);
    }

    #[test]
    fn update_rejects_unowned_garage_as_headquarters() {
        let error = mutation_target(
            &garage("garage.los_angeles"),
            &GarageOperation::Update,
            None,
            true,
        )
        .unwrap_err();
        assert_eq!(error, "garage_not_owned");
    }

    #[test]
    fn active_context_rejects_profile_or_save_switch() {
        let profile_state = AppProfileState::default();
        *profile_state.current_profile.lock().unwrap() = Some("profile-a".to_string());
        *profile_state.current_save.lock().unwrap() = Some("save-a".to_string());
        let expected = ActiveSaveSelection {
            profile_path: Some("profile-a".to_string()),
            save_path: Some("save-a".to_string()),
        };
        assert!(ensure_active_context(&profile_state, &expected, "ets2").is_ok());

        *profile_state.current_save.lock().unwrap() = Some("save-b".to_string());
        assert_eq!(
            ensure_active_context(&profile_state, &expected, "ets2").unwrap_err(),
            "save_changed_since_load:active_selection_changed"
        );
    }

    #[test]
    fn unresolved_assignments_block_mutation() {
        let mut target = garage("garage.berlin");
        target
            .warnings
            .push("garage_truck_reference_unresolved:garage.berlin:0".to_string());
        let error =
            validate_mutation_target(&target, &GarageOperation::Update, Some(GarageSize::Small))
                .unwrap_err();
        assert_eq!(error, "garage_downgrade_has_unresolved_references");
    }

    #[test]
    fn ambiguous_or_duplicate_assignments_block_mutation() {
        for warning in [
            "garage_truck_reference_ambiguous:garage.berlin:0",
            "garage_driver_reference_duplicate:garage.berlin:0",
            "garage_slot_assignment_inconsistent:garage.berlin:0:driver_without_truck",
        ] {
            let mut target = garage("garage.berlin");
            target.warnings.push(warning.to_string());
            let error = validate_mutation_target(
                &target,
                &GarageOperation::Update,
                Some(GarageSize::Large),
            )
            .unwrap_err();
            assert_eq!(error, "garage_has_unresolved_references");
        }
    }

    #[test]
    fn inconsistent_unowned_headquarters_is_blocked() {
        let content = SAMPLE.replace(" hq_city: berlin", " hq_city: los_angeles");
        let target = parse_garages_from_sii(&content)
            .unwrap()
            .garages
            .into_iter()
            .find(|garage| garage.garage_id == "garage.los_angeles")
            .unwrap();
        let error =
            validate_mutation_target(&target, &GarageOperation::Purchase, Some(GarageSize::Large))
                .unwrap_err();
        assert_eq!(error, "garage_state_invalid");
    }

    #[test]
    fn rollback_readback_requires_original_hash_and_valid_structure() {
        let original_hash = sha256_hex_bytes(SAMPLE.as_bytes());
        assert!(verify_restored_content(&original_hash, SAMPLE).is_ok());

        assert_eq!(
            verify_restored_content("wrong-hash", SAMPLE).unwrap_err(),
            "rollback_failed:verification_mismatch"
        );

        let damaged = "SiiNunit\n{\ngarage : garage.berlin {\n status: 2\n}\n}";
        let damaged_hash = sha256_hex_bytes(damaged.as_bytes());
        assert_eq!(
            verify_restored_content(&damaged_hash, damaged).unwrap_err(),
            "rollback_failed:invalid_restored_save"
        );
    }
}
