use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::features::backup::service as backup_service;
use crate::features::vehicles::resolve_active_save_from_snapshot;
use crate::models::trucks::ParsedTruck;
use crate::shared::decrypt::decrypt_cached_with_cache;
use crate::shared::paths::game_sii_from_save;
use crate::shared::sii_parser::parse_trucks_from_sii;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::cache::{CurrentTruckCacheEntry, TruckChangeSessionCache};
use super::models::{
    ApplyTruckChangeResult, DriverAssignmentEvidence, DriverAssignmentSource, DriverDisplayInfo,
    DriverResolutionDiagnostics, DriverResolutionError, ResolvedDriverAssignment,
    TruckChangePreview, TruckChangeSession, TruckInventoryItem, TruckSwitchList, TruckSwitchMode,
};
use super::parser::{
    assignment_conflicts_from_blocks, extract_field_value, graph_dangling_accessories,
    normalize_sii_unit_id, parse_truck_save, parse_unit_blocks,
};
use super::validator::validate_truck_switch_content;
use super::writer::{
    TemporaryRollbackSnapshot, set_unit_array_value, set_unit_field_value, unit_field_exists,
    write_verified_content,
};

pub fn list_owned_trucks_for_switch_from_content(
    save_path: &Path,
    content: &str,
) -> TruckSwitchList {
    let parsed = parse_truck_save(content);
    let mut warnings = Vec::new();
    if parsed.active_truck_id.is_none() {
        warnings.push("missing_my_truck_pointer".to_string());
    }

    TruckSwitchList {
        save_path: save_path.display().to_string(),
        file_hash: sha256_hex(content.as_bytes()),
        active_truck_id: parsed.active_truck_id,
        trucks: parsed.trucks,
        diagnostics: parsed.diagnostics,
        warnings,
    }
}

pub fn initialize_truck_change_session_from_content(
    profile_id: &str,
    save_path: &Path,
    content: &str,
    session_cache: &TruckChangeSessionCache,
) -> Result<TruckChangeSession, String> {
    let file_hash = sha256_hex(content.as_bytes());
    if let Some(entry) = session_cache.get(profile_id, save_path, &file_hash) {
        crate::dev_log!("[truck_change] current truck cache hit");
        return Ok(session_from_cache_entry(save_path, file_hash, entry));
    }

    let list = list_owned_trucks_for_switch_from_content(save_path, content);
    let current_truck = list
        .active_truck_id
        .as_deref()
        .and_then(|active| find_inventory_item(&list.trucks, active))
        .ok_or_else(|| "current_truck_not_found".to_string())?;
    let session = TruckChangeSession {
        save_path: save_path.display().to_string(),
        save_hash: list.file_hash.clone(),
        current_truck,
        owned_trucks: list.trucks,
        diagnostics: Some(list.diagnostics),
        warnings: list.warnings,
    };

    session_cache.store(CurrentTruckCacheEntry::from_session(
        profile_id.to_string(),
        save_path.to_path_buf(),
        &session,
    ));
    crate::dev_log!("[truck_change] current truck cached");
    crate::dev_log!("[truck_change] owned trucks cached");
    Ok(session)
}

pub fn preview_active_truck_switch_from_content(
    save_path: &Path,
    content: &str,
    target_truck_id: &str,
    expected_file_hash: &str,
) -> TruckChangePreview {
    let parsed = parse_truck_save(content);
    let actual_hash = sha256_hex(content.as_bytes());
    let current_truck = parsed
        .active_truck_id
        .as_deref()
        .and_then(|active| find_inventory_item(&parsed.trucks, active))
        .unwrap_or_else(|| missing_inventory_item("_missing_current"));
    let target_truck = find_inventory_item(&parsed.trucks, target_truck_id)
        .unwrap_or_else(|| missing_inventory_item(target_truck_id));
    let driver_resolution = resolve_target_driver(&parsed, &target_truck);
    let affected_driver = driver_resolution
        .as_ref()
        .ok()
        .and_then(|resolved| resolved.as_ref())
        .map(|resolved| resolved.driver.clone());
    let driver_resolution_error = driver_resolution.as_ref().err().cloned();
    let mode = if target_truck.assigned_driver_id.is_some()
        || target_truck.requires_driver_swap
        || affected_driver.is_some()
        || driver_resolution_error.is_some()
    {
        TruckSwitchMode::DriverSwap
    } else {
        TruckSwitchMode::FreeTruck
    };
    let mut warnings = Vec::new();
    let mut can_apply = true;

    if expected_file_hash != actual_hash {
        warnings.push("save_changed_since_session".to_string());
        can_apply = false;
    }
    if parsed.active_truck_id.is_none() {
        warnings.push("current_truck_not_found".to_string());
        can_apply = false;
    }
    if !parsed
        .trucks
        .iter()
        .any(|truck| truck.truck_id.eq_ignore_ascii_case(target_truck_id))
    {
        warnings.push("target_truck_not_found".to_string());
        can_apply = false;
    }
    if parsed
        .active_truck_id
        .as_deref()
        .map(|active| active.eq_ignore_ascii_case(target_truck_id))
        .unwrap_or(false)
    {
        warnings.push("target_already_active".to_string());
        can_apply = false;
    }
    if let Some(error) = driver_resolution_error.as_ref() {
        warnings.push("driver_assignment_unresolved".to_string());
        can_apply = false;
        crate::dev_log!(
            "[truck_change] unresolved driver diagnostics generated resolution_error={}",
            error.code()
        );
    }
    if mode == TruckSwitchMode::DriverSwap {
        crate::dev_log!("[truck_change] driver swap preview started");
        let duplicate_assignments = duplicate_driver_or_truck_assignments(&parsed);
        if !duplicate_assignments.is_empty() {
            warnings.push("duplicate_assignment_detected".to_string());
            can_apply = false;
        }
        if !can_swap_driver_to_current_player_truck(&parsed, &current_truck) {
            warnings.push("driver_swap_assignment_missing".to_string());
            can_apply = false;
        }
        if let Ok(Some(resolved)) = driver_resolution.as_ref() {
            crate::dev_log!(
                "[truck_change] driver resolved resolution_source={:?}",
                resolved.source
            );
        }
    }

    match parsed.truck_graphs.get(target_truck_id) {
        Some(graph) => {
            let dangling = graph_dangling_accessories(graph, &parsed.unit_ids);
            if !dangling.is_empty() {
                warnings.push("dangling_vehicle_references".to_string());
                can_apply = false;
            }
        }
        None => {
            warnings.push("target_vehicle_block_missing".to_string());
            can_apply = false;
        }
    }

    let _ = save_path;
    let assignment_warnings = inspect_assignment_references(content, target_truck_id);
    warnings.extend(assignment_warnings);
    warnings.sort();
    warnings.dedup();
    let error_code = if can_apply {
        None
    } else {
        Some(preview_error_code(&warnings))
    };
    let diagnostics = driver_resolution_error
        .as_ref()
        .map(|error| driver_resolution_diagnostics(&parsed, &target_truck, Some(error)));
    if mode == TruckSwitchMode::DriverSwap {
        if can_apply {
            crate::dev_log!("[truck_change] driver swap preview completed");
        } else {
            crate::dev_log!("[truck_change] driver swap preview failed");
        }
    }

    TruckChangePreview {
        mode: mode.clone(),
        current_truck: current_truck.clone(),
        target_truck: target_truck.clone(),
        current_player_truck: current_truck.clone(),
        selected_truck: target_truck.clone(),
        affected_driver,
        driver_receives_truck: if mode == TruckSwitchMode::DriverSwap {
            Some(current_truck)
        } else {
            None
        },
        warnings,
        error_code,
        diagnostics,
        expected_file_hash: actual_hash,
        can_apply,
    }
}

pub fn apply_active_truck_switch_transaction(
    save_path_arg: Option<String>,
    target_truck_id: String,
    expected_file_hash: String,
    create_persistent_backup: bool,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    truck_change_cache: &TruckChangeSessionCache,
) -> Result<ApplyTruckChangeResult, String> {
    let profile_id = current_profile_id(profile_state)?;
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
    decrypt_cache.invalidate_path(&game_path);
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    let actual_hash = sha256_hex(content.as_bytes());
    if actual_hash != expected_file_hash {
        return Err("save_changed_since_preview".to_string());
    }

    if parse_truck_save(&content).active_truck_id.is_none() {
        return Err("current_truck_not_found".to_string());
    }

    let preview = preview_active_truck_switch_from_content(
        &game_path,
        &content,
        &target_truck_id,
        &expected_file_hash,
    );
    if !preview.can_apply {
        return Err(format!("preview_blocked:{}", preview.warnings.join(",")));
    }

    let backup_id = if create_persistent_backup {
        let backup_targets = backup_service::recommended_targets(&game_path);
        let backup = backup_service::create_backup_for_targets(
            profile_state,
            "active truck switch",
            &backup_targets,
        )?;
        Some(backup.backup_id.clone())
    } else {
        crate::dev_log!("[truck_change] persistent backup skipped by user");
        None
    };
    let mut rollback = TemporaryRollbackSnapshot::create(&game_path)?;

    let result = (|| {
        let switch_plan = apply_switch_to_content(&content, &target_truck_id)?;
        let temp_validation = validate_truck_switch_content(
            &switch_plan.content,
            &target_truck_id,
            switch_plan.affected_driver_id.as_deref(),
            Some(&switch_plan.previous_truck_id),
        );
        if !temp_validation.success {
            return Err(format!(
                "temporary_validation_failed:{}",
                temp_validation.errors.join(",")
            ));
        }

        write_verified_content(&game_path, &switch_plan.content, |candidate| {
            let validation = validate_truck_switch_content(
                candidate,
                &target_truck_id,
                switch_plan.affected_driver_id.as_deref(),
                Some(&switch_plan.previous_truck_id),
            );
            if validation.success {
                Ok(())
            } else {
                Err(format!(
                    "temporary_parse_or_validation_failed:{}",
                    validation.errors.join(",")
                ))
            }
        })?;

        invalidate_after_write(&game_path, profile_cache, decrypt_cache);
        truck_change_cache.invalidate_save(&profile_id, &game_path);
        let reloaded = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
        let _production_trucks: Vec<ParsedTruck> = parse_trucks_from_sii(&reloaded);
        let validation = validate_truck_switch_content(
            &reloaded,
            &target_truck_id,
            switch_plan.affected_driver_id.as_deref(),
            Some(&switch_plan.previous_truck_id),
        );
        if !validation.success {
            return Err(format!(
                "post_write_verification_failed:{}",
                validation.errors.join(",")
            ));
        }

        let file_hash_after = sha256_hex(reloaded.as_bytes());
        let refreshed_session = initialize_truck_change_session_from_content(
            &profile_id,
            &game_path,
            &reloaded,
            truck_change_cache,
        )?;
        rollback.cleanup()?;
        if switch_plan.affected_driver_id.is_some() {
            crate::dev_log!("[truck_change] semantic driver swap validation passed");
        }
        crate::dev_log!("[truck_change] production reload completed");
        crate::dev_log!("[truck_change] refreshed session cached");
        Ok(ApplyTruckChangeResult {
            success: true,
            backup_id: backup_id.clone(),
            persistent_backup_created: backup_id.is_some(),
            temporary_rollback_used: true,
            temporary_rollback_cleaned: rollback.cleaned(),
            previous_truck_id: switch_plan.previous_truck_id,
            active_truck_id: target_truck_id.clone(),
            affected_driver_id: switch_plan.affected_driver_id,
            driver_received_truck_id: switch_plan.driver_received_truck_id,
            file_hash_before: actual_hash,
            file_hash_after,
            validation,
            refreshed_session,
        })
    })();

    match result {
        Ok(value) => Ok(value),
        Err(error) => {
            let rollback_result = rollback.restore();
            invalidate_after_write(&game_path, profile_cache, decrypt_cache);
            truck_change_cache.invalidate_save(&profile_id, &game_path);
            let _ = rollback.cleanup();
            match rollback_result {
                Ok(_) => Err(format!("{};temporary_rollback_restored", error)),
                Err(rollback_error) => Err(format!("{};rollback_failed:{}", error, rollback_error)),
            }
        }
    }
}

pub struct SwitchApplyPlan {
    pub content: String,
    pub previous_truck_id: String,
    pub affected_driver_id: Option<String>,
    pub driver_received_truck_id: Option<String>,
}

pub fn apply_switch_to_content(
    content: &str,
    target_truck_id: &str,
) -> Result<SwitchApplyPlan, String> {
    let parsed = parse_truck_save(content);
    let previous_truck_id = parsed
        .active_truck_id
        .clone()
        .ok_or_else(|| "current_truck_not_found".to_string())?;
    if previous_truck_id.eq_ignore_ascii_case(target_truck_id) {
        return Err("target_already_active".to_string());
    }
    let target = parsed
        .trucks
        .iter()
        .find(|truck| truck.truck_id.eq_ignore_ascii_case(target_truck_id))
        .ok_or_else(|| "target_truck_not_found".to_string())?;
    let target_graph = parsed
        .truck_graphs
        .get(target_truck_id)
        .ok_or_else(|| "target_vehicle_block_missing".to_string())?;
    let dangling = graph_dangling_accessories(target_graph, &parsed.unit_ids);
    if !dangling.is_empty() {
        return Err(format!(
            "dangling_vehicle_references:{}",
            dangling.join(",")
        ));
    }
    let player_id = parsed
        .player_id
        .as_deref()
        .ok_or_else(|| "player_not_found".to_string())?;
    let resolved_driver_assignment = resolve_target_driver(&parsed, target)
        .map_err(|_| "driver_assignment_unresolved".to_string())?;
    let needs_driver_swap = target.requires_driver_swap
        || target.assigned_driver_id.is_some()
        || resolved_driver_assignment.is_some();

    if needs_driver_swap {
        if unsupported_player_job(content, player_id, &previous_truck_id) {
            return Err("unsupported_external_job_assignment".to_string());
        }
        if resolved_driver_assignment.is_none() {
            return Err("driver_assignment_unresolved".to_string());
        }
        if !duplicate_driver_or_truck_assignments(&parsed).is_empty() {
            return Err("duplicate_assignment_detected".to_string());
        }
        if !can_swap_driver_to_current_player_truck(
            &parsed,
            &missing_or_active(&parsed, &previous_truck_id),
        ) {
            return Err("driver_swap_assignment_missing".to_string());
        }
    }
    let mut updated = content.to_string();

    let (next, changed_my_truck) =
        set_unit_field_value(&updated, player_id, "my_truck", target_truck_id)?;
    if !changed_my_truck {
        return Err("missing_my_truck_pointer".to_string());
    }
    updated = next;

    if unit_field_exists(&updated, player_id, "assigned_truck") {
        let (next, _) =
            set_unit_field_value(&updated, player_id, "assigned_truck", target_truck_id)?;
        updated = next;
    }

    if let Some(current_job_id) = player_current_job(content, player_id) {
        if player_job_company_truck(content, &current_job_id)
            .map(|truck| truck.eq_ignore_ascii_case(&previous_truck_id))
            .unwrap_or(false)
        {
            let (next, _) =
                set_unit_field_value(&updated, &current_job_id, "company_truck", target_truck_id)?;
            updated = next;
        }
    }

    let mut affected_driver_id = None;
    let mut driver_received_truck_id = None;

    if needs_driver_swap {
        let previous_assignment = parsed
            .garage_assignments
            .get(&normalize_sii_unit_id(&previous_truck_id))
            .ok_or_else(|| "driver_swap_assignment_missing".to_string())?;
        let target_assignment = parsed
            .garage_assignments
            .get(&normalize_sii_unit_id(target_truck_id))
            .ok_or_else(|| "driver_swap_assignment_missing".to_string())?;
        let driver_id = resolved_driver_assignment
            .as_ref()
            .map(|resolved| resolved.driver.driver_id.clone())
            .ok_or_else(|| "driver_assignment_unresolved".to_string())?;

        let (next, changed_previous_slot) = set_unit_array_value(
            &updated,
            &previous_assignment.garage_id,
            "drivers",
            previous_assignment.slot_index,
            &driver_id,
        )?;
        if !changed_previous_slot {
            return Err("driver_swap_assignment_missing".to_string());
        }
        updated = next;

        let (next, changed_target_slot) = set_unit_array_value(
            &updated,
            &target_assignment.garage_id,
            "drivers",
            target_assignment.slot_index,
            "null",
        )?;
        if !changed_target_slot {
            return Err("driver_swap_assignment_missing".to_string());
        }
        updated = next;

        if unit_field_exists(&updated, &driver_id, "assigned_truck") {
            let (next, _) =
                set_unit_field_value(&updated, &driver_id, "assigned_truck", &previous_truck_id)?;
            updated = next;
        }

        affected_driver_id = Some(driver_id);
        driver_received_truck_id = Some(previous_truck_id.clone());
        crate::dev_log!("[truck_change] player/driver assignment swap prepared");
        crate::dev_log!("[truck_change] driver swap transaction prepared");
    }

    Ok(SwitchApplyPlan {
        content: updated,
        previous_truck_id,
        affected_driver_id,
        driver_received_truck_id,
    })
}

pub fn resolve_game_sii_path(
    save_path_arg: Option<String>,
    profile_state: &AppProfileState,
) -> Result<PathBuf, String> {
    let save_path = match save_path_arg {
        Some(path) if !path.trim().is_empty() => path,
        _ => resolve_active_save_from_snapshot(
            profile_state.current_save.lock().unwrap().clone(),
            profile_state.current_profile.lock().unwrap().clone(),
        )?,
    };
    Ok(game_sii_from_save(Path::new(&save_path)))
}

pub fn read_switch_list(
    save_path_arg: Option<String>,
    profile_state: &AppProfileState,
    decrypt_cache: &DecryptCache,
) -> Result<TruckSwitchList, String> {
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
    decrypt_cache.invalidate_path(&game_path);
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    Ok(list_owned_trucks_for_switch_from_content(
        &game_path, &content,
    ))
}

pub fn read_truck_change_session(
    save_path_arg: Option<String>,
    profile_state: &AppProfileState,
    decrypt_cache: &DecryptCache,
    session_cache: &TruckChangeSessionCache,
) -> Result<TruckChangeSession, String> {
    let profile_id = current_profile_id(profile_state)?;
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
    crate::dev_log!("[truck_change] session initialization started");
    decrypt_cache.invalidate_path(&game_path);
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    initialize_truck_change_session_from_content(&profile_id, &game_path, &content, session_cache)
}

pub fn read_switch_preview(
    save_path_arg: Option<String>,
    target_truck_id: String,
    expected_file_hash: String,
    profile_state: &AppProfileState,
    decrypt_cache: &DecryptCache,
) -> Result<TruckChangePreview, String> {
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
    decrypt_cache.invalidate_path(&game_path);
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    Ok(preview_active_truck_switch_from_content(
        &game_path,
        &content,
        &target_truck_id,
        &expected_file_hash,
    ))
}

pub fn read_content_for_path(
    save_path: &str,
    decrypt_cache: &DecryptCache,
) -> Result<(PathBuf, String), String> {
    let game_path = game_sii_from_save(Path::new(save_path));
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    Ok((game_path, content))
}

fn invalidate_after_write(
    game_path: &Path,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
) {
    decrypt_cache.invalidate_path(game_path);
    profile_cache.invalidate_vehicle_data();
    profile_cache.invalidate_save_data();
}

fn current_profile_id(profile_state: &AppProfileState) -> Result<String, String> {
    profile_state
        .current_profile
        .lock()
        .unwrap()
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "profile_not_selected".to_string())
}

fn session_from_cache_entry(
    save_path: &Path,
    save_hash: String,
    entry: CurrentTruckCacheEntry,
) -> TruckChangeSession {
    TruckChangeSession {
        save_path: save_path.display().to_string(),
        save_hash,
        current_truck: entry.truck,
        owned_trucks: entry.owned_trucks,
        diagnostics: None,
        warnings: Vec::new(),
    }
}

fn preview_error_code(warnings: &[String]) -> String {
    const PRIORITY: &[&str] = &[
        "save_changed_since_session",
        "current_truck_not_found",
        "target_truck_not_found",
        "target_already_active",
        "driver_assignment_unresolved",
        "driver_swap_assignment_missing",
        "duplicate_assignment_detected",
        "dangling_vehicle_references",
        "target_vehicle_block_missing",
    ];
    PRIORITY
        .iter()
        .find(|code| warnings.iter().any(|warning| warning == *code))
        .map(|code| (*code).to_string())
        .or_else(|| warnings.first().cloned())
        .unwrap_or_else(|| "preview_blocked".to_string())
}

fn find_inventory_item(items: &[TruckInventoryItem], truck_id: &str) -> Option<TruckInventoryItem> {
    items
        .iter()
        .find(|truck| truck.truck_id.eq_ignore_ascii_case(truck_id))
        .cloned()
}

fn resolve_target_driver(
    parsed: &super::parser::ParsedTruckSave,
    target_truck: &TruckInventoryItem,
) -> Result<Option<ResolvedDriverAssignment>, DriverResolutionError> {
    let target_truck_id_normalized = normalize_sii_unit_id(&target_truck.truck_id);
    crate::dev_log!(
        "[truck_change] resolving target driver target_truck_id={}",
        target_truck.truck_id
    );

    let garage_assignment = parsed.garage_assignments.get(&target_truck_id_normalized);
    if let Some(assignment) = garage_assignment {
        crate::dev_log!(
            "[truck_change] garage slot assignment found target_truck_id={} garage_id={} slot_index={} garage_driver_id={}",
            target_truck.truck_id,
            assignment.garage_id,
            assignment.slot_index,
            assignment.driver_id.as_deref().unwrap_or("null")
        );
    }

    let drivers_pointing_to_target =
        drivers_pointing_to_target_truck(parsed, &target_truck_id_normalized);
    if drivers_pointing_to_target.len() > 1 {
        crate::dev_log!(
            "[truck_change] unresolved driver diagnostics generated target_truck_id={} drivers_pointing_to_target={}",
            target_truck.truck_id,
            drivers_pointing_to_target.len()
        );
        return Err(DriverResolutionError::AmbiguousDriverAssignment);
    }

    let garage_driver_id_raw = garage_assignment
        .and_then(|assignment| assignment.driver_id.clone())
        .or_else(|| target_truck.assigned_driver_id.clone());
    let garage_driver_id_normalized = garage_driver_id_raw
        .as_deref()
        .map(normalize_sii_unit_id)
        .filter(|value| !value.is_empty());

    if let Some(driver_id_normalized) = garage_driver_id_normalized.as_deref() {
        crate::dev_log!(
            "[truck_change] normalized driver lookup completed target_truck_id={} garage_driver_id={}",
            target_truck.truck_id,
            garage_driver_id_raw.as_deref().unwrap_or("")
        );
        if let Some(driver) = parsed.driver_infos.get(driver_id_normalized) {
            if let Some(pointing_driver) = drivers_pointing_to_target.first() {
                if pointing_driver.normalized_id != driver.normalized_id {
                    crate::dev_log!(
                        "[truck_change] conflicting driver assignment detected target_truck_id={} garage_driver_id={} truck_reference_driver_id={}",
                        target_truck.truck_id,
                        driver.driver_id,
                        pointing_driver.driver_id
                    );
                    return Err(DriverResolutionError::ConflictingDriverAssignment);
                }
            }

            if driver
                .current_truck_id_normalized
                .as_deref()
                .map(|truck_id| truck_id != target_truck_id_normalized)
                .unwrap_or(false)
            {
                crate::dev_log!(
                    "[truck_change] conflicting driver assignment detected target_truck_id={} driver_id={} assigned_truck={}",
                    target_truck.truck_id,
                    driver.driver_id,
                    driver.current_truck_id.as_deref().unwrap_or("")
                );
                return Err(DriverResolutionError::ConflictingDriverAssignment);
            }

            let source = if driver.current_truck_id_normalized.is_some() {
                DriverAssignmentSource::ReconciledGarageAndDriver
            } else {
                DriverAssignmentSource::GarageSlot
            };
            return Ok(Some(resolved_driver_assignment(
                driver.clone(),
                source,
                garage_assignment,
                vec![DriverAssignmentEvidence {
                    source: DriverAssignmentSource::GarageSlot,
                    driver_id: garage_driver_id_raw,
                    truck_id: Some(target_truck.truck_id.clone()),
                    garage_id: garage_assignment.map(|assignment| assignment.garage_id.clone()),
                    slot_index: garage_assignment.map(|assignment| assignment.slot_index),
                    detail: "garage_slot_driver_id".to_string(),
                }],
            )));
        }

        crate::dev_log!(
            "[truck_change] driver block lookup missed target_truck_id={} garage_driver_id={}",
            target_truck.truck_id,
            garage_driver_id_raw.as_deref().unwrap_or("")
        );
    }

    crate::dev_log!(
        "[truck_change] searching drivers by assigned truck target_truck_id={}",
        target_truck.truck_id
    );
    if let Some(driver) = drivers_pointing_to_target.first() {
        crate::dev_log!(
            "[truck_change] driver resolved by truck reference target_truck_id={} driver_id={}",
            target_truck.truck_id,
            driver.driver_id
        );
        return Ok(Some(resolved_driver_assignment(
            (*driver).clone(),
            DriverAssignmentSource::DriverAssignedTruck,
            garage_assignment,
            vec![DriverAssignmentEvidence {
                source: DriverAssignmentSource::DriverAssignedTruck,
                driver_id: Some(driver.driver_id.clone()),
                truck_id: driver.current_truck_id.clone(),
                garage_id: garage_assignment.map(|assignment| assignment.garage_id.clone()),
                slot_index: garage_assignment.map(|assignment| assignment.slot_index),
                detail: "driver_assigned_truck".to_string(),
            }],
        )));
    }

    if garage_driver_id_normalized.is_some() {
        return Err(DriverResolutionError::MissingDriverBlock);
    }

    Ok(None)
}

fn resolved_driver_assignment(
    driver: DriverDisplayInfo,
    source: DriverAssignmentSource,
    garage_assignment: Option<&super::models::GarageSlotAssignment>,
    evidence: Vec<DriverAssignmentEvidence>,
) -> ResolvedDriverAssignment {
    ResolvedDriverAssignment {
        driver,
        source,
        garage_id: garage_assignment.map(|assignment| assignment.garage_id.clone()),
        slot_index: garage_assignment.map(|assignment| assignment.slot_index),
        evidence,
    }
}

fn drivers_pointing_to_target_truck(
    parsed: &super::parser::ParsedTruckSave,
    target_truck_id_normalized: &str,
) -> Vec<DriverDisplayInfo> {
    let mut drivers = parsed
        .driver_infos
        .values()
        .filter(|driver| {
            driver
                .current_truck_id_normalized
                .as_deref()
                .map(|truck_id| truck_id == target_truck_id_normalized)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    drivers.sort_by(|left, right| left.normalized_id.cmp(&right.normalized_id));
    drivers
}

fn driver_resolution_diagnostics(
    parsed: &super::parser::ParsedTruckSave,
    target_truck: &TruckInventoryItem,
    error: Option<&DriverResolutionError>,
) -> DriverResolutionDiagnostics {
    let target_truck_id_normalized = normalize_sii_unit_id(&target_truck.truck_id);
    let garage_assignment = parsed.garage_assignments.get(&target_truck_id_normalized);
    let garage_driver_id_raw = garage_assignment
        .and_then(|assignment| assignment.driver_id.clone())
        .or_else(|| target_truck.assigned_driver_id.clone());
    let garage_driver_id_normalized = garage_driver_id_raw
        .as_deref()
        .map(normalize_sii_unit_id)
        .filter(|value| !value.is_empty());
    let exact_driver_id_match = garage_driver_id_normalized
        .as_deref()
        .map(|driver_id| parsed.driver_infos.contains_key(driver_id))
        .unwrap_or(false);
    let raw_exact_driver_id_match = garage_driver_id_raw
        .as_deref()
        .map(|driver_id| {
            parsed
                .driver_infos
                .values()
                .any(|driver| driver.raw_id == driver_id)
        })
        .unwrap_or(false);
    let case_insensitive_match = garage_driver_id_raw
        .as_deref()
        .map(|driver_id| {
            !raw_exact_driver_id_match
                && parsed
                    .driver_infos
                    .values()
                    .any(|driver| driver.raw_id.eq_ignore_ascii_case(driver_id))
        })
        .unwrap_or(false);
    let drivers_pointing_to_target_truck =
        drivers_pointing_to_target_truck(parsed, &target_truck_id_normalized)
            .into_iter()
            .map(|driver| driver.driver_id)
            .collect::<Vec<_>>();
    let similar_driver_ids = garage_driver_id_raw
        .as_deref()
        .map(|driver_id| {
            find_similar_driver_ids(
                driver_id,
                parsed
                    .driver_infos
                    .values()
                    .map(|driver| driver.raw_id.as_str()),
            )
        })
        .unwrap_or_default();
    let diagnostics = DriverResolutionDiagnostics {
        target_truck_id: target_truck.truck_id.clone(),
        target_truck_id_normalized,
        garage_id: garage_assignment.map(|assignment| assignment.garage_id.clone()),
        garage_slot_index: garage_assignment.map(|assignment| assignment.slot_index),
        garage_driver_id_raw,
        garage_driver_id_normalized,
        recognized_driver_count: parsed.driver_infos.len(),
        recognized_driver_unit_types: parsed.driver_diagnostics.recognized_unit_types.clone(),
        exact_driver_id_match,
        case_insensitive_match,
        drivers_pointing_to_target_truck,
        similar_driver_ids,
        garage_vehicle_count: garage_assignment.map(|assignment| assignment.garage_vehicle_count),
        garage_driver_count: garage_assignment.map(|assignment| assignment.garage_driver_count),
        arrays_have_matching_indices: garage_assignment
            .map(|assignment| assignment.arrays_have_matching_indices)
            .unwrap_or(true),
        resolution_error: error.map(|error| error.code().to_string()),
    };
    crate::dev_log!(
        "[truck_change] unresolved driver diagnostics generated target_truck_id={} garage_id={} slot_index={} garage_driver_id={} recognized_drivers={} drivers_pointing_to_target={} arrays_have_matching_indices={} resolution_error={}",
        diagnostics.target_truck_id,
        diagnostics.garage_id.as_deref().unwrap_or(""),
        diagnostics
            .garage_slot_index
            .map(|index| index.to_string())
            .unwrap_or_default(),
        diagnostics.garage_driver_id_raw.as_deref().unwrap_or(""),
        diagnostics.recognized_driver_count,
        diagnostics.drivers_pointing_to_target_truck.len(),
        diagnostics.arrays_have_matching_indices,
        diagnostics.resolution_error.as_deref().unwrap_or("")
    );
    diagnostics
}

fn find_similar_driver_ids<'a>(
    expected: &str,
    available: impl Iterator<Item = &'a str>,
) -> Vec<String> {
    let expected_normalized = normalize_sii_unit_id(expected);
    if expected_normalized.is_empty() {
        return Vec::new();
    }
    let mut similar = available
        .filter_map(|candidate| {
            let candidate_normalized = normalize_sii_unit_id(candidate);
            if candidate_normalized == expected_normalized {
                return None;
            }
            let common_prefix = expected_normalized
                .chars()
                .zip(candidate_normalized.chars())
                .take_while(|(left, right)| left == right)
                .count();
            if common_prefix >= 8
                && common_prefix * 2 >= expected_normalized.len().min(candidate_normalized.len())
            {
                Some(candidate.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    similar.sort();
    similar.truncate(8);
    similar
}

fn can_swap_driver_to_current_player_truck(
    parsed: &super::parser::ParsedTruckSave,
    current_truck: &TruckInventoryItem,
) -> bool {
    parsed
        .garage_assignments
        .get(&normalize_sii_unit_id(&current_truck.truck_id))
        .map(|assignment| assignment.driver_id.is_none())
        .unwrap_or(false)
}

fn duplicate_driver_or_truck_assignments(parsed: &super::parser::ParsedTruckSave) -> Vec<String> {
    let blocks = parsed.unit_blocks.values().cloned().collect::<Vec<_>>();
    assignment_conflicts_from_blocks(&blocks)
}

fn missing_or_active(
    parsed: &super::parser::ParsedTruckSave,
    truck_id: &str,
) -> TruckInventoryItem {
    find_inventory_item(&parsed.trucks, truck_id)
        .unwrap_or_else(|| missing_inventory_item(truck_id))
}

fn missing_inventory_item(truck_id: &str) -> TruckInventoryItem {
    TruckInventoryItem {
        truck_id: truck_id.to_string(),
        display_index: 0,
        brand: None,
        model: None,
        raw_license_plate: None,
        display_license_plate: None,
        license_plate: None,
        garage_id: None,
        garage_display_name: None,
        assigned_garage: None,
        assigned_driver_id: None,
        driver_display_name: None,
        country_code: None,
        country_display_name: None,
        is_active: false,
        is_switchable: false,
        blocked_reason: Some("truck_not_found".to_string()),
        requires_driver_swap: false,
        engine_data_path: None,
        transmission_data_path: None,
        accessory_count: 0,
    }
}

fn inspect_assignment_references(content: &str, target_truck_id: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let parsed = parse_truck_save(content);
    if parsed
        .garage_assignments
        .contains_key(&normalize_sii_unit_id(target_truck_id))
    {
        warnings.push("target_has_garage_assignment".to_string());
    }
    warnings
}

fn player_current_job(content: &str, player_id: &str) -> Option<String> {
    parse_unit_blocks(content)
        .into_iter()
        .find(|block| block.id.eq_ignore_ascii_case(player_id))
        .and_then(|block| extract_field_value(&block.raw_block, "current_job"))
}

fn player_job_company_truck(content: &str, job_id: &str) -> Option<String> {
    parse_unit_blocks(content)
        .into_iter()
        .find(|block| block.id.eq_ignore_ascii_case(job_id))
        .and_then(|block| extract_field_value(&block.raw_block, "company_truck"))
}

fn unsupported_player_job(content: &str, player_id: &str, previous_truck_id: &str) -> bool {
    let Some(current_job_id) = player_current_job(content, player_id) else {
        return false;
    };
    let Some(block) = parse_unit_blocks(content)
        .into_iter()
        .find(|block| block.id.eq_ignore_ascii_case(&current_job_id))
    else {
        return false;
    };
    let job_references_previous = player_job_company_truck(content, &current_job_id)
        .map(|truck| truck.eq_ignore_ascii_case(previous_truck_id))
        .unwrap_or(false);
    if !job_references_previous {
        return false;
    }

    block
        .raw_block
        .lines()
        .map(|line| line.to_ascii_lowercase())
        .any(|line| line.contains("external") || line.contains("online"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_switch_to_content, initialize_truck_change_session_from_content,
        list_owned_trucks_for_switch_from_content, preview_active_truck_switch_from_content,
    };
    use crate::features::truck_change::cache::TruckChangeSessionCache;
    use crate::features::truck_change::models::TruckSwitchMode;
    use crate::features::truck_change::validator::validate_truck_switch_content;
    use std::path::Path;

    fn fixture() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 assigned_truck: _nameless.truck.active
 my_truck: _nameless.truck.active
 current_job: _nameless.job
 trucks: 3
 trucks[0]: _nameless.truck.active
 trucks[1]: _nameless.truck.free1
 trucks[2]: _nameless.truck.free2
}
player_job : _nameless.job {
 company_truck: _nameless.truck.active
}
vehicle : _nameless.truck.active {
 accessories: 1
 accessories[0]: _nameless.acc.active
}
vehicle_accessory : _nameless.acc.active {
 data_path: "/def/vehicle/truck/scania.s_2016/data.sii"
}
vehicle : _nameless.truck.free1 {
 accessories: 1
 accessories[0]: _nameless.acc.free1
}
vehicle_accessory : _nameless.acc.free1 {
 data_path: "/def/vehicle/truck/scania.s_2016/data.sii"
}
vehicle : _nameless.truck.free2 {
 accessories: 1
 accessories[0]: _nameless.acc.free2
}
vehicle_accessory : _nameless.acc.free2 {
 data_path: "/def/vehicle/truck/man.tgx/data.sii"
}
garage : garage.berlin {
 vehicles: 3
 vehicles[0]: _nameless.truck.active
 vehicles[1]: _nameless.truck.free1
 vehicles[2]: _nameless.truck.free2
 drivers: 3
 drivers[0]: null
 drivers[1]: null
 drivers[2]: null
}
driver : driver.1 {
 name: "Max"
 surname: "Mustermann"
 assigned_truck: _nameless.truck.free2
}
}
"#
    }

    #[test]
    fn identical_models_remain_separately_selectable() {
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), fixture());
        assert_eq!(list.trucks.len(), 3);
        assert_ne!(list.trucks[0].truck_id, list.trucks[1].truck_id);
        assert_eq!(list.trucks[0].brand, list.trucks[1].brand);
    }

    #[test]
    fn active_truck_is_detected_by_id() {
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), fixture());
        assert_eq!(
            list.active_truck_id.as_deref(),
            Some("_nameless.truck.active")
        );
        assert!(list.trucks[0].is_active);
        assert!(!list.trucks[1].is_active);
    }

    #[test]
    fn apply_switch_survives_full_reload_validation() {
        let plan = apply_switch_to_content(fixture(), "_nameless.truck.free1").unwrap();
        let validation =
            validate_truck_switch_content(&plan.content, "_nameless.truck.free1", None, None);
        assert!(validation.success, "{:?}", validation.errors);
        assert!(plan.content.contains(" my_truck: _nameless.truck.free1"));
        assert!(
            plan.content
                .contains(" assigned_truck: _nameless.truck.free1")
        );
        assert!(
            plan.content
                .contains(" company_truck: _nameless.truck.free1")
        );
    }

    #[test]
    fn preview_blocks_changed_save_hash() {
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), fixture());
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            fixture(),
            "_nameless.truck.free1",
            &format!("{}-changed", list.file_hash),
        );
        assert!(!preview.can_apply);
        assert!(
            preview
                .warnings
                .contains(&"save_changed_since_session".to_string())
        );
    }

    #[test]
    fn free_truck_preview_returns_free_truck_mode_and_can_apply() {
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), fixture());
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            fixture(),
            "_nameless.truck.free1",
            &list.file_hash,
        );

        assert_eq!(preview.mode, TruckSwitchMode::FreeTruck);
        assert!(preview.can_apply);
        assert_eq!(preview.error_code, None);
    }

    #[test]
    fn driver_truck_is_switchable_with_driver_swap() {
        let content = fixture().replace("drivers[2]: null", "drivers[2]: driver.1");
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let driver_truck = list
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.free2")
            .unwrap();
        assert!(driver_truck.is_switchable);
        assert!(driver_truck.requires_driver_swap);
        assert_eq!(
            driver_truck.driver_display_name.as_deref(),
            Some("Max Mustermann")
        );
    }

    #[test]
    fn driver_truck_preview_returns_driver_swap_mode_and_can_apply() {
        let content = fixture().replace("drivers[2]: null", "drivers[2]: driver.1");
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.free2",
            &list.file_hash,
        );

        assert_eq!(preview.mode, TruckSwitchMode::DriverSwap);
        assert!(preview.can_apply);
        assert_eq!(
            preview
                .affected_driver
                .as_ref()
                .map(|driver| driver.driver_id.as_str()),
            Some("driver.1")
        );
    }

    #[test]
    fn unresolved_driver_reference_blocks_driver_swap_preview() {
        let content = fixture()
            .replace("drivers[2]: null", "drivers[2]: driver.missing")
            .replace("driver : driver.1", "driver : driver.other")
            .replace(
                "assigned_truck: _nameless.truck.free2",
                "assigned_truck: _nameless.truck.other",
            );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.free2",
            &list.file_hash,
        );

        assert_eq!(preview.mode, TruckSwitchMode::DriverSwap);
        assert!(!preview.can_apply);
        assert_eq!(
            preview.error_code.as_deref(),
            Some("driver_assignment_unresolved")
        );
        assert!(preview.diagnostics.is_some());
    }

    #[test]
    fn driver_truck_preview_resolves_normalized_garage_driver_id() {
        let content = fixture().replace("drivers[2]: null", "drivers[2]: \"DRIVER.1\"");
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.free2",
            &list.file_hash,
        );

        assert_eq!(preview.mode, TruckSwitchMode::DriverSwap);
        assert!(preview.can_apply);
        assert_eq!(
            preview
                .affected_driver
                .as_ref()
                .map(|driver| driver.driver_id.as_str()),
            Some("driver.1")
        );
    }

    #[test]
    fn driver_truck_preview_resolves_by_assigned_truck_reference() {
        let content = fixture().replace("drivers[2]: null", "drivers[2]: driver.missing");
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.free2",
            &list.file_hash,
        );

        assert_eq!(preview.mode, TruckSwitchMode::DriverSwap);
        assert!(preview.can_apply);
        assert_eq!(
            preview
                .affected_driver
                .as_ref()
                .map(|driver| driver.driver_id.as_str()),
            Some("driver.1")
        );
    }

    #[test]
    fn two_drivers_pointing_to_same_target_blocks_preview() {
        let content = fixture().replace(
            "driver : driver.1 {",
            "driver : driver.2 {\n name: \"Erika\"\n assigned_truck: _nameless.truck.free2\n}\ndriver : driver.1 {",
        );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.free2",
            &list.file_hash,
        );

        assert_eq!(preview.mode, TruckSwitchMode::DriverSwap);
        assert!(!preview.can_apply);
        assert_eq!(
            preview
                .diagnostics
                .as_ref()
                .and_then(|diagnostics| diagnostics.resolution_error.as_deref()),
            Some("ambiguous_driver_assignment")
        );
    }

    #[test]
    fn conflicting_garage_and_driver_reference_blocks_preview() {
        let content = fixture()
            .replace("drivers[2]: null", "drivers[2]: driver.2")
            .replace(
                "driver : driver.1 {",
                "driver : driver.2 {\n name: \"Erika\"\n assigned_truck: _nameless.truck.free1\n}\ndriver : driver.1 {",
            );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.free2",
            &list.file_hash,
        );

        assert!(!preview.can_apply);
        assert_eq!(
            preview
                .diagnostics
                .as_ref()
                .and_then(|diagnostics| diagnostics.resolution_error.as_deref()),
            Some("conflicting_driver_assignment")
        );
    }

    #[test]
    fn similar_driver_id_is_only_reported_as_diagnostic() {
        let content = fixture()
            .replace("drivers[2]: null", "drivers[2]: driver.10")
            .replace("driver : driver.1", "driver : driver.11")
            .replace(
                "assigned_truck: _nameless.truck.free2",
                "assigned_truck: _nameless.truck.other",
            );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.free2",
            &list.file_hash,
        );

        assert!(!preview.can_apply);
        assert_eq!(preview.affected_driver, None);
        assert_eq!(
            preview
                .diagnostics
                .as_ref()
                .map(|diagnostics| diagnostics.similar_driver_ids.clone())
                .unwrap_or_default(),
            vec!["driver.11".to_string()]
        );
    }

    #[test]
    fn driver_truck_swap_assigns_driver_to_previous_player_truck() {
        let content = fixture().replace("drivers[2]: null", "drivers[2]: driver.1");
        let plan = apply_switch_to_content(&content, "_nameless.truck.free2").unwrap();
        assert_eq!(plan.affected_driver_id.as_deref(), Some("driver.1"));
        assert_eq!(
            plan.driver_received_truck_id.as_deref(),
            Some("_nameless.truck.active")
        );
        assert!(plan.content.contains(" my_truck: _nameless.truck.free2"));
        assert!(plan.content.contains(" drivers[0]: driver.1"));
        assert!(plan.content.contains(" drivers[2]: null"));
        assert!(
            plan.content
                .contains(" assigned_truck: _nameless.truck.active")
        );
        let validation = validate_truck_switch_content(
            &plan.content,
            "_nameless.truck.free2",
            Some("driver.1"),
            Some("_nameless.truck.active"),
        );
        assert!(validation.success, "{:?}", validation.errors);
    }

    #[test]
    fn session_initialization_returns_current_truck_and_owned_trucks() {
        let cache = TruckChangeSessionCache::default();
        let session = initialize_truck_change_session_from_content(
            "profile-a",
            Path::new("game.sii"),
            fixture(),
            &cache,
        )
        .unwrap();

        assert_eq!(session.current_truck.truck_id, "_nameless.truck.active");
        assert_eq!(session.owned_trucks.len(), 3);
        assert_eq!(session.save_hash.len(), 64);
    }

    #[test]
    fn session_after_driver_swap_contains_new_player_truck() {
        let cache = TruckChangeSessionCache::default();
        let content = fixture().replace("drivers[2]: null", "drivers[2]: driver.1");
        let plan = apply_switch_to_content(&content, "_nameless.truck.free2").unwrap();
        let session = initialize_truck_change_session_from_content(
            "profile-a",
            Path::new("game.sii"),
            &plan.content,
            &cache,
        )
        .unwrap();

        assert_eq!(session.current_truck.truck_id, "_nameless.truck.free2");
        let previous_player_truck = session
            .owned_trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.active")
            .unwrap();
        assert_eq!(
            previous_player_truck.assigned_driver_id.as_deref(),
            Some("driver.1")
        );
    }

    #[test]
    fn duplicate_driver_assignment_blocks_driver_swap_preview_and_apply() {
        let content = fixture()
            .replace("drivers[0]: null", "drivers[0]: driver.1")
            .replace("drivers[2]: null", "drivers[2]: driver.1");
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.free2",
            &list.file_hash,
        );

        assert!(!preview.can_apply);
        assert!(
            preview
                .warnings
                .contains(&"duplicate_assignment_detected".to_string())
        );
        match apply_switch_to_content(&content, "_nameless.truck.free2") {
            Ok(_) => panic!("duplicate assignment should block apply"),
            Err(error) => assert_eq!(error, "duplicate_assignment_detected"),
        }
    }

    #[test]
    fn unreferenced_vehicle_blocks_are_not_listed_as_owned_trucks() {
        let content = fixture().replace(
            "garage : garage.berlin",
            r#"vehicle : _nameless.truck.job_market {
 accessories: 1
 accessories[0]: _nameless.acc.job_market
}
vehicle_accessory : _nameless.acc.job_market {
 data_path: "/def/vehicle/truck/iveco.hiway/data.sii"
}
garage : garage.berlin"#,
        );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        assert_eq!(list.trucks.len(), 3);
        assert!(
            list.trucks
                .iter()
                .all(|truck| truck.truck_id != "_nameless.truck.job_market")
        );
        assert_eq!(list.diagnostics.owned_trucks, 3);
        assert_eq!(list.diagnostics.excluded_job_vehicles, 1);
    }

    #[test]
    fn ownership_collection_filters_204_vehicle_blocks_to_148_owned_trucks() {
        let mut content = String::from(
            "SiiNunit\n{\neconomy : _nameless.economy {\n player: _nameless.player\n}\nplayer : _nameless.player {\n my_truck: _nameless.truck.0\n trucks: 148\n",
        );
        for index in 0..148 {
            content.push_str(&format!(" trucks[{}]: _nameless.truck.{}\n", index, index));
        }
        content.push_str("}\n");
        for index in 0..204 {
            content.push_str(&format!(
                "vehicle : _nameless.truck.{} {{\n accessories: 1\n accessories[0]: _nameless.acc.{}\n}}\nvehicle_accessory : _nameless.acc.{} {{\n data_path: \"/def/vehicle/truck/scania.s_2016/data.sii\"\n}}\n",
                index, index, index
            ));
        }
        content.push_str("}\n");

        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        assert_eq!(list.diagnostics.total_vehicle_blocks, 204);
        assert_eq!(list.diagnostics.candidate_trucks, 204);
        assert_eq!(list.trucks.len(), 148);
        assert_eq!(list.diagnostics.owned_trucks, 148);
        assert_eq!(list.diagnostics.excluded_unreferenced, 56);
    }
}
