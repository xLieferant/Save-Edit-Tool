use std::collections::{BTreeMap, BTreeSet};
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
    ApplyTruckChangeResult, CurrentTruckPointer, CurrentTruckPointerKind, DriverAssignmentEvidence,
    DriverAssignmentSource, DriverDisplayInfo, DriverResolutionDiagnostics, DriverResolutionError,
    DriverResolutionKind, GarageSlotAssignment, ResolvedDriverAssignment, TruckAssignmentContext,
    TruckAssignmentKind, TruckChangePreview, TruckChangeSession, TruckGarageSlotReference,
    PlayerVehicleSlotAssignment, TruckInventoryItem, TruckReferenceMatch, TruckSwapPreviewDetails,
    TruckSwitchList, TruckSwitchMode,
};
use super::parser::{
    assignment_conflicts_from_blocks, extract_array_entries, extract_array_values,
    extract_field_value, garage_driver_ref_is_unique, graph_dangling_accessories, is_null_ref,
    normalize_sii_unit_id, parse_truck_save, parse_unit_blocks, resolve_current_truck_pointer,
};
use super::validator::validate_truck_switch_content;
use super::writer::{
    set_unit_field_value, unit_field_exists, write_verified_content, TemporaryRollbackSnapshot,
};

pub fn list_owned_trucks_for_switch_from_content(
    save_path: &Path,
    content: &str,
) -> TruckSwitchList {
    let parsed = parse_truck_save(content);
    let mut trucks = parsed.trucks.clone();
    annotate_truck_switchability(&parsed, &mut trucks);
    let mut warnings = Vec::new();
    if parsed.truck_order.is_empty() {
        warnings.push("owned_trucks_missing".to_string());
    }
    if resolve_current_truck_pointer(&parsed).is_err() {
        warnings.push("current_truck_unresolved".to_string());
    }
    if !parsed
        .diagnostics
        .player_truck_reference_missing_vehicle_blocks
        .is_empty()
    {
        warnings.push("player_truck_reference_missing_vehicle_block".to_string());
    }

    TruckSwitchList {
        save_path: save_path.display().to_string(),
        file_hash: sha256_hex(content.as_bytes()),
        active_truck_id: parsed.active_truck_id.clone(),
        trucks,
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
    if list.trucks.is_empty() {
        return Err("owned_trucks_missing".to_string());
    }
    let current_truck = list
        .active_truck_id
        .as_deref()
        .and_then(|active| find_inventory_item(&list.trucks, active))
        .ok_or_else(|| "current_truck_unresolved".to_string())?;
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
    let current_pointer = resolve_current_truck_pointer(&parsed).ok();
    let current_truck = current_pointer
        .as_ref()
        .and_then(|pointer| find_inventory_item(&parsed.trucks, &pointer.truck_id))
        .unwrap_or_else(|| missing_inventory_item("_missing_current"));
    let target_truck = find_inventory_item(&parsed.trucks, target_truck_id)
        .unwrap_or_else(|| missing_inventory_item(target_truck_id));
    let affected_driver = unique_driver_assigned_to_truck(&parsed, target_truck_id).ok().flatten();
    let mode = if affected_driver.is_some() {
        TruckSwitchMode::DriverSwap
    } else {
        TruckSwitchMode::FreeTruck
    };
    let swap_plan = build_truck_swap_preview_details(&parsed, current_pointer.as_ref(), target_truck_id);
    let mut warnings = Vec::new();
    let mut can_apply = true;

    if expected_file_hash != actual_hash {
        warnings.push("save_changed_since_session".to_string());
        can_apply = false;
    }
    if current_pointer.is_none() {
        warnings.push("current_truck_unresolved".to_string());
        can_apply = false;
    }
    let target_owned = player_trucks_contains(&parsed, target_truck_id);
    if !target_owned {
        warnings.push("target_truck_not_owned".to_string());
        can_apply = false;
    }
    if current_pointer
        .as_ref()
        .map(|pointer| pointer.truck_id.eq_ignore_ascii_case(target_truck_id))
        .unwrap_or(false)
    {
        warnings.push("target_already_active".to_string());
        can_apply = false;
    }

    if let Some(pointer) = current_pointer.as_ref() {
        if !parsed
            .truck_graphs
            .values()
            .any(|graph| normalize_sii_unit_id(&graph.vehicle_id) == normalize_sii_unit_id(&pointer.truck_id))
        {
            warnings.push("current_vehicle_block_missing".to_string());
            can_apply = false;
        }
    }

    match find_truck_graph_case_insensitive(&parsed, target_truck_id) {
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

    if let Some(pointer) = current_pointer.as_ref() {
        if target_owned && !pointer.truck_id.eq_ignore_ascii_case(target_truck_id) {
            if let Err(reason) = resolve_truck_switch_write_plan(&parsed, &pointer.truck_id, target_truck_id) {
                warnings.push(reason.to_string());
                can_apply = false;
            }
        }
    }

    let _ = save_path;
    warnings.sort();
    warnings.dedup();
    let error_code = if can_apply {
        None
    } else {
        Some(preview_error_code(&warnings))
    };
    let diagnostics = None;

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
        swap_plan: Some(swap_plan),
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

    let parsed_before_preview = parse_truck_save(&content);
    let current_pointer_before_preview = resolve_current_truck_pointer(&parsed_before_preview)?;

    let preview = preview_active_truck_switch_from_content(
        &game_path,
        &content,
        &target_truck_id,
        &expected_file_hash,
    );
    if !preview.can_apply {
        return Err(format!("preview_blocked:{}", preview.warnings.join(",")));
    }
    if !preview
        .current_truck
        .truck_id
        .eq_ignore_ascii_case(&current_pointer_before_preview.truck_id)
    {
        return Err("current_truck_changed_since_preview".to_string());
    }

    let backup_id = if create_persistent_backup {
        let backup_targets = backup_service::recommended_targets(&game_path);
        let backup = backup_service::create_backup_for_targets(
            profile_state,
            "active truck switch",
            &backup_targets,
        )
        .map_err(|error| format!("backup_failed:{}", error))?;
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
                "write_verification_failed:{}",
                temp_validation.errors.join(",")
            ));
        }
        let temp_parsed = parse_truck_save(&switch_plan.content);
        let temp_semantic_errors = verify_truck_switch_after_write(
            &parsed_before_preview,
            &temp_parsed,
            &target_truck_id,
            &switch_plan.previous_truck_id,
        );
        if !temp_semantic_errors.is_empty() {
            return Err(format!(
                "write_verification_failed:{}",
                temp_semantic_errors.join(",")
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
                    "write_verification_failed:{}",
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
                "write_verification_failed:{}",
                validation.errors.join(",")
            ));
        }
        let parsed_after = parse_truck_save(&reloaded);
        let semantic_errors = verify_truck_switch_after_write(
            &parsed_before_preview,
            &parsed_after,
            &target_truck_id,
            &switch_plan.previous_truck_id,
        );
        if !semantic_errors.is_empty() {
            return Err(format!(
                "write_verification_failed:{}",
                semantic_errors.join(",")
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

#[derive(Debug, Clone)]
struct TruckSwitchWritePlan {
    current_slot: PlayerVehicleSlotAssignment,
    target_slot: Option<PlayerVehicleSlotAssignment>,
    target_driver: Option<DriverDisplayInfo>,
    write_case: &'static str,
    old_truck_destination: String,
}

#[derive(Debug, Clone)]
struct ResolvedGarageDriverSlot {
    assignment: GarageSlotAssignment,
    write_vehicle_slot: bool,
    source: &'static str,
}

#[derive(Debug, Clone)]
struct DriverSwapGaragePlan {
    target_slot: Option<TruckGarageSlotReference>,
    previous_slot: Option<ResolvedGarageDriverSlot>,
}

pub fn apply_switch_to_content(
    content: &str,
    target_truck_id: &str,
) -> Result<SwitchApplyPlan, String> {
    let parsed = parse_truck_save(content);
    let current_pointer = resolve_current_truck_pointer(&parsed)?;
    let previous_truck_id = current_pointer.truck_id.clone();
    if previous_truck_id.eq_ignore_ascii_case(target_truck_id) {
        return Err("target_already_active".to_string());
    }
    if !player_trucks_contains(&parsed, target_truck_id) {
        return Err("target_truck_not_owned".to_string());
    }
    if find_truck_graph_case_insensitive(&parsed, &previous_truck_id).is_none() {
        return Err("current_vehicle_block_missing".to_string());
    }
    let target_graph = find_truck_graph_case_insensitive(&parsed, target_truck_id)
        .ok_or_else(|| "target_vehicle_block_missing".to_string())?;
    let dangling = graph_dangling_accessories(target_graph, &parsed.unit_ids);
    if !dangling.is_empty() {
        return Err(format!(
            "dangling_vehicle_references:{}",
            dangling.join(",")
        ));
    }
    let switch_plan = resolve_truck_switch_write_plan(&parsed, &previous_truck_id, target_truck_id)
        .map_err(|reason| reason.to_string())?;
    let mut updated = content.to_string();

    let (next, changed_current_slot) = set_unit_field_value(
        &updated,
        &switch_plan.current_slot.slot_id,
        "vehicle",
        target_truck_id,
    )?;
    if !changed_current_slot {
        return Err("current_slot_unresolved".to_string());
    }
    updated = next;

    if current_pointer.writable
        && !matches!(
            current_pointer.kind,
            CurrentTruckPointerKind::PlayerAssignedVehicles
                | CurrentTruckPointerKind::FallbackPlayerVehicles
        )
    {
        let (next, changed_current_pointer) =
            set_current_truck_pointer_value(&updated, &current_pointer, target_truck_id)?;
        if !changed_current_pointer {
            return Err(current_truck_pointer_write_error(&current_pointer).to_string());
        }
        updated = next;
    }
    if let Some(player_id) = parsed.player_id.as_deref() {
        for field in ["assigned_truck", "my_truck"] {
            let Some(value) = parsed
                .unit_blocks
                .values()
                .find(|block| normalize_sii_unit_id(&block.id) == normalize_sii_unit_id(player_id))
                .and_then(|block| extract_field_value(&block.raw_block, field))
            else {
                continue;
            };
            if normalize_sii_unit_id(&value) != normalize_sii_unit_id(&previous_truck_id) {
                continue;
            }
            let (next, changed_field) =
                set_unit_field_value(&updated, player_id, field, target_truck_id)?;
            if changed_field {
                updated = next;
            }
        }
    }

    let mut affected_driver_id = None;
    let mut driver_received_truck_id = None;

    if let Some(target_slot) = switch_plan.target_slot.as_ref() {
        let (next, changed_target_slot) =
            set_unit_field_value(&updated, &target_slot.slot_id, "vehicle", &previous_truck_id)?;
        if !changed_target_slot {
            return Err("old_truck_destination_missing".to_string());
        }
        updated = next;
    }
    if let Some(target_driver) = switch_plan.target_driver.as_ref() {
        let driver_field = target_driver
            .current_truck_field
            .clone()
            .or_else(|| driver_truck_field_in_content(&updated, &target_driver.driver_id))
            .ok_or_else(|| "old_truck_destination_missing".to_string())?;
        let (next, changed_driver_field) = set_unit_field_value(
            &updated,
            &target_driver.driver_id,
            &driver_field,
            &previous_truck_id,
        )?;
        if !changed_driver_field {
            return Err("old_truck_destination_missing".to_string());
        }
        updated = next;
        affected_driver_id = Some(target_driver.driver_id.clone());
        driver_received_truck_id = Some(previous_truck_id.clone());
    }

    if let Some(player_id) = parsed.player_id.as_deref() {
        if let Some(current_job_id) = player_current_job(content, player_id) {
            if player_job_company_truck(content, &current_job_id)
                .map(|truck| truck.eq_ignore_ascii_case(&previous_truck_id))
                .unwrap_or(false)
            {
                let (next, _) = set_unit_field_value(
                    &updated,
                    &current_job_id,
                    "company_truck",
                    target_truck_id,
                )?;
                updated = next;
            }
        }
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
        diagnostics: entry.diagnostics,
        warnings: Vec::new(),
    }
}

fn preview_error_code(warnings: &[String]) -> String {
    const PRIORITY: &[&str] = &[
        "save_changed_since_session",
        "owned_trucks_missing",
        "current_truck_unresolved",
        "target_truck_not_owned",
        "target_vehicle_block_missing",
        "current_vehicle_block_missing",
        "current_slot_unresolved",
        "old_truck_destination_missing",
        "target_already_active",
        "write_verification_failed",
        "backup_failed",
        "driver_assignment_ambiguous",
        "driver_assignment_unresolved",
        "driver_swap_assignment_missing",
        "duplicate_assignment_detected",
        "dangling_vehicle_references",
    ];
    PRIORITY
        .iter()
        .find(|code| warnings.iter().any(|warning| warning == *code))
        .map(|code| (*code).to_string())
        .or_else(|| warnings.first().cloned())
        .unwrap_or_else(|| "preview_blocked".to_string())
}

fn player_trucks_contains(parsed: &super::parser::ParsedTruckSave, truck_id: &str) -> bool {
    let normalized = normalize_sii_unit_id(truck_id);
    parsed
        .truck_order
        .iter()
        .any(|candidate| normalize_sii_unit_id(candidate) == normalized)
}

fn find_truck_graph_case_insensitive<'a>(
    parsed: &'a super::parser::ParsedTruckSave,
    truck_id: &str,
) -> Option<&'a super::models::TruckGraph> {
    let normalized = normalize_sii_unit_id(truck_id);
    parsed
        .truck_graphs
        .values()
        .find(|graph| normalize_sii_unit_id(&graph.vehicle_id) == normalized)
}

fn player_vehicle_slot_for_truck(
    parsed: &super::parser::ParsedTruckSave,
    truck_id: &str,
) -> Option<PlayerVehicleSlotAssignment> {
    parsed
        .player_vehicle_assignments
        .get(&normalize_sii_unit_id(truck_id))
        .cloned()
}

fn unique_driver_assigned_to_truck(
    parsed: &super::parser::ParsedTruckSave,
    truck_id: &str,
) -> Result<Option<DriverDisplayInfo>, &'static str> {
    let normalized = normalize_sii_unit_id(truck_id);
    let mut drivers = parsed
        .driver_infos
        .values()
        .filter(|driver| driver.unit_type != "driver_player")
        .filter(|driver| {
            driver
                .current_truck_id_normalized
                .as_deref()
                .map(|candidate| candidate == normalized)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    drivers.sort_by(|left, right| left.normalized_id.cmp(&right.normalized_id));
    drivers.dedup_by(|left, right| left.normalized_id == right.normalized_id);
    match drivers.len() {
        0 => Ok(None),
        1 => Ok(drivers.pop()),
        _ => Err("old_truck_destination_missing"),
    }
}

fn resolve_truck_switch_write_plan(
    parsed: &super::parser::ParsedTruckSave,
    previous_truck_id: &str,
    target_truck_id: &str,
) -> Result<TruckSwitchWritePlan, &'static str> {
    let current_slot =
        player_vehicle_slot_for_truck(parsed, previous_truck_id).ok_or("current_slot_unresolved")?;
    let target_slot = player_vehicle_slot_for_truck(parsed, target_truck_id);
    let target_driver = unique_driver_assigned_to_truck(parsed, target_truck_id)
        .map_err(|_| "old_truck_destination_missing")?;
    if let Some(target_slot) = target_slot {
        let write_case = if target_driver.is_some() {
            "player_vehicle_slot_and_driver"
        } else {
            "player_vehicle_slot"
        };
        return Ok(TruckSwitchWritePlan {
            current_slot,
            old_truck_destination: format!("player_vehicles:{}", target_slot.slot_id),
            target_slot: Some(target_slot),
            target_driver,
            write_case,
        });
    }

    let Some(target_driver) = target_driver else {
        return Ok(TruckSwitchWritePlan {
            current_slot,
            old_truck_destination: "unassigned_owned".to_string(),
            target_slot: None,
            target_driver: None,
            write_case: "target_unassigned_owned",
        });
    };
    Ok(TruckSwitchWritePlan {
        current_slot,
        old_truck_destination: format!("driver:{}", target_driver.driver_id),
        target_slot: None,
        target_driver: Some(target_driver),
        write_case: "driver_assigned_truck",
    })
}

fn build_truck_swap_preview_details(
    parsed: &super::parser::ParsedTruckSave,
    current_pointer: Option<&CurrentTruckPointer>,
    target_truck_id: &str,
) -> TruckSwapPreviewDetails {
    let target_slot = player_vehicle_slot_for_truck(parsed, target_truck_id);
    let target_driver_result = unique_driver_assigned_to_truck(parsed, target_truck_id);
    let target_driver = target_driver_result.clone().ok().flatten();
    let target_is_free =
        target_slot.is_none() && target_driver.is_none() && target_driver_result.is_ok();
    let plan = current_pointer
        .and_then(|pointer| {
            resolve_truck_switch_write_plan(parsed, &pointer.truck_id, target_truck_id).ok()
        });
    let target_location = if target_slot.is_some() {
        Some("player_vehicle_slot".to_string())
    } else if target_driver.is_some() {
        Some("driver_assigned_truck".to_string())
    } else if target_is_free {
        Some("unassigned_owned".to_string())
    } else {
        None
    };

    TruckSwapPreviewDetails {
        current_truck_id: current_pointer.map(|pointer| pointer.truck_id.clone()),
        target_truck_id: target_truck_id.to_string(),
        target_location,
        old_truck_destination: plan
            .as_ref()
            .map(|plan| plan.old_truck_destination.clone()),
        target_is_free,
        target_player_vehicle_slot_id: target_slot.as_ref().map(|slot| slot.slot_id.clone()),
        target_player_vehicle_slot_index: target_slot.as_ref().and_then(|slot| slot.slot_index),
        target_driver_id: target_driver.as_ref().map(|driver| driver.driver_id.clone()),
        write_case: plan.as_ref().map(|plan| plan.write_case.to_string()),
        can_write_safely: plan.is_some(),
    }
}

fn verify_truck_switch_after_write(
    before: &super::parser::ParsedTruckSave,
    after: &super::parser::ParsedTruckSave,
    target_truck_id: &str,
    previous_truck_id: &str,
) -> Vec<String> {
    let mut errors = Vec::new();
    let before_owned = before
        .truck_order
        .iter()
        .map(|truck_id| normalize_sii_unit_id(truck_id))
        .collect::<Vec<_>>();
    let after_owned = after
        .truck_order
        .iter()
        .map(|truck_id| normalize_sii_unit_id(truck_id))
        .collect::<Vec<_>>();
    if before_owned.len() != after_owned.len() {
        errors.push("owned_truck_count_changed".to_string());
    }
    if before_owned != after_owned {
        errors.push("owned_truck_list_changed".to_string());
    }
    if !player_trucks_contains(after, target_truck_id) {
        errors.push("target_truck_missing_from_player_trucks".to_string());
    }
    if !player_trucks_contains(after, previous_truck_id) {
        errors.push("old_truck_missing_from_player_trucks".to_string());
    }
    if find_truck_graph_case_insensitive(after, previous_truck_id).is_none() {
        errors.push("current_vehicle_block_missing".to_string());
    }
    for slot in &after.player_vehicle_slots {
        let Some(truck_id) = slot.truck_id.as_deref() else {
            continue;
        };
        if find_truck_graph_case_insensitive(after, truck_id).is_none() {
            errors.push(format!("invalid_player_vehicle_ref:{}", slot.slot_id));
        }
    }
    for driver in after.driver_infos.values() {
        let Some(truck_id) = driver.current_truck_id.as_deref() else {
            continue;
        };
        if find_truck_graph_case_insensitive(after, truck_id).is_none() {
            errors.push(format!("invalid_driver_truck_ref:{}", driver.driver_id));
        }
    }
    errors.sort();
    errors.dedup();
    errors
}

fn driver_resolution_warning_code(error: &DriverResolutionError) -> &'static str {
    match error {
        DriverResolutionError::AmbiguousGarageDriverRef
        | DriverResolutionError::AmbiguousDriverAssignment => "driver_assignment_ambiguous",
        _ => "driver_assignment_unresolved",
    }
}

fn driver_resolution_apply_error_code(error: &DriverResolutionError) -> &'static str {
    match error {
        DriverResolutionError::AmbiguousGarageDriverRef
        | DriverResolutionError::AmbiguousDriverAssignment => "driver_assignment_ambiguous",
        _ => "driver_assignment_unresolved",
    }
}

fn driver_swap_plan_warning_code(reason: &str) -> &'static str {
    match reason {
        "multiple_driver_refs" | "ambiguous_garage_slots" => "driver_assignment_ambiguous",
        "missing_driver_ref" | "missing_vehicle_block" | "unknown_assignment" => {
            "driver_assignment_unresolved"
        }
        _ => "driver_swap_assignment_missing",
    }
}

const DRIVER_TRUCK_REFERENCE_FIELDS: &[&str] =
    &["assigned_truck", "assigned_vehicle", "truck", "vehicle"];

fn inspect_truck_assignment_context(
    parsed: &super::parser::ParsedTruckSave,
    truck_id: &str,
) -> TruckAssignmentContext {
    let truck_id_normalized = normalize_sii_unit_id(truck_id);
    let vehicle_unit_found = parsed
        .truck_graphs
        .values()
        .any(|graph| normalize_sii_unit_id(&graph.vehicle_id) == truck_id_normalized);
    let in_player_trucks = parsed
        .truck_order
        .iter()
        .any(|candidate| normalize_sii_unit_id(candidate) == truck_id_normalized);
    let current_pointer = parsed.current_truck_pointer.as_ref();
    let active_for_player = current_pointer
        .map(|pointer| normalize_sii_unit_id(&pointer.truck_id) == truck_id_normalized)
        .unwrap_or(false);
    let player_assigned_vehicles_unit =
        current_pointer.and_then(|pointer| pointer.referenced_player_vehicle_unit_id.clone());
    let player_vehicles_vehicle = if active_for_player {
        parsed
            .current_truck_diagnostics
            .assigned_vehicles_vehicle_raw
            .clone()
    } else {
        None
    };
    let reverse_references = reverse_references_to_truck(parsed, &truck_id_normalized);
    let driver_references = reverse_references
        .iter()
        .filter(|reference| is_driver_unit_type(&reference.unit_type))
        .cloned()
        .collect::<Vec<_>>();
    let other_references = reverse_references
        .iter()
        .filter(|reference| !is_driver_unit_type(&reference.unit_type))
        .cloned()
        .collect::<Vec<_>>();
    let garage_references = garage_slot_references_to_truck(parsed, &truck_id_normalized);
    let garage_slot_candidate_count = garage_references.len();
    let mut ai_driver_candidates = BTreeSet::new();

    for driver in parsed.driver_infos.values() {
        if driver
            .current_truck_id_normalized
            .as_deref()
            .map(|candidate| candidate == truck_id_normalized)
            .unwrap_or(false)
            && !is_player_driver_ref(parsed, &driver.driver_id)
        {
            ai_driver_candidates.insert(driver.driver_id.clone());
        }
    }

    let ai_driver_candidate_count = ai_driver_candidates.len();
    let driver_ref = if ai_driver_candidate_count == 1 {
        ai_driver_candidates.iter().next().cloned()
    } else {
        None
    };
    let garage_ref = if garage_references.len() == 1 {
        Some(garage_references[0].garage_id.clone())
    } else {
        None
    };
    let garage_slot_index = if garage_references.len() == 1 {
        Some(garage_references[0].slot_index)
    } else {
        None
    };
    let has_garage_driver_ref = garage_references
        .iter()
        .any(|reference| reference.driver_ref.is_some());
    let garage_driver_ref_conflicts = driver_ref
        .as_deref()
        .map(|driver_id| {
            garage_references.iter().any(|reference| {
                reference
                    .driver_ref
                    .as_deref()
                    .map(|garage_driver_id| {
                        normalize_sii_unit_id(garage_driver_id) != normalize_sii_unit_id(driver_id)
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    let garage_arrays_inconsistent = garage_references
        .iter()
        .any(|reference| !reference.arrays_have_matching_indices);
    let mut evidence = Vec::new();
    if vehicle_unit_found {
        evidence.push("vehicle_block_found".to_string());
    }
    if in_player_trucks {
        evidence.push("player.trucks[]".to_string());
    }
    if active_for_player {
        evidence.push("player_active_assignment".to_string());
    }
    if player_assigned_vehicles_unit.is_some() {
        evidence.push("player.assigned_vehicles".to_string());
        evidence.push("player_vehicles.vehicle".to_string());
    }
    evidence.extend(
        driver_references
            .iter()
            .map(|reference| format!("driver:{}", reference_summary(reference))),
    );
    evidence.extend(garage_references.iter().map(|reference| {
        format!(
            "garage:{}#{} vehicle={} driver={}",
            reference.garage_id,
            reference.slot_index,
            reference.vehicle_ref.as_deref().unwrap_or("null"),
            reference.driver_ref.as_deref().unwrap_or("null")
        )
    }));
    evidence.sort();
    evidence.dedup();

    let (assignment_kind, unsafe_reason) = if active_for_player {
        (
            TruckAssignmentKind::PlayerActive,
            Some("player_active".to_string()),
        )
    } else if ai_driver_candidate_count > 1 {
        (
            TruckAssignmentKind::Ambiguous,
            Some("multiple_driver_refs".to_string()),
        )
    } else if garage_arrays_inconsistent {
        (
            TruckAssignmentKind::Ambiguous,
            Some("ambiguous_garage_slots".to_string()),
        )
    } else if garage_slot_candidate_count > 1 {
        (
            TruckAssignmentKind::Ambiguous,
            Some("ambiguous_garage_slots".to_string()),
        )
    } else if garage_driver_ref_conflicts {
        (
            TruckAssignmentKind::Ambiguous,
            Some("conflicting_driver_refs".to_string()),
        )
    } else if ai_driver_candidate_count == 1 {
        (
            TruckAssignmentKind::AiDriverAssigned,
            Some("ai_driver_resolved".to_string()),
        )
    } else if in_player_trucks && vehicle_unit_found && !has_garage_driver_ref {
        (
            TruckAssignmentKind::UnassignedOwnedTruck,
            Some("unassigned_owned_truck".to_string()),
        )
    } else if !vehicle_unit_found {
        (
            TruckAssignmentKind::Unknown,
            Some("missing_vehicle_block".to_string()),
        )
    } else if has_garage_driver_ref {
        (
            TruckAssignmentKind::Unknown,
            Some("missing_driver_ref".to_string()),
        )
    } else {
        (
            TruckAssignmentKind::Unknown,
            Some("unknown_assignment".to_string()),
        )
    };

    TruckAssignmentContext {
        truck_id: truck_id.to_string(),
        truck_id_normalized,
        vehicle_unit_found,
        in_player_trucks,
        active_for_player,
        player_assigned_vehicles_unit,
        player_vehicles_vehicle,
        assignment_kind,
        driver_ref,
        garage_ref,
        garage_slot_index,
        driver_references,
        garage_references,
        reverse_references,
        other_references,
        ai_driver_candidate_count,
        garage_slot_candidate_count,
        evidence,
        unsafe_reason,
    }
}

fn reverse_references_to_truck(
    parsed: &super::parser::ParsedTruckSave,
    truck_id_normalized: &str,
) -> Vec<TruckReferenceMatch> {
    let mut references = Vec::new();

    for block in parsed.unit_blocks.values() {
        for line in block.raw_block.lines().skip(1) {
            let Some((field_name, value)) = parse_reference_line(line) else {
                continue;
            };
            if normalize_sii_unit_id(&value) != truck_id_normalized {
                continue;
            }
            references.push(TruckReferenceMatch {
                unit_type: block.unit_type.clone(),
                unit_id: block.id.clone(),
                field_name,
                value,
            });
        }
    }

    references.sort_by(|left, right| {
        (
            left.unit_type.as_str(),
            left.unit_id.as_str(),
            left.field_name.as_str(),
            left.value.as_str(),
        )
            .cmp(&(
                right.unit_type.as_str(),
                right.unit_id.as_str(),
                right.field_name.as_str(),
                right.value.as_str(),
            ))
    });
    references.dedup();
    references
}

fn parse_reference_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
        return None;
    }
    let (field, value) = trimmed.split_once(':')?;
    let field = field.trim();
    if field.is_empty() || field.contains(' ') {
        return None;
    }
    let value = value
        .split("//")
        .next()
        .unwrap_or(value)
        .trim()
        .trim_end_matches(',')
        .trim()
        .to_string();
    if value.is_empty() || is_null_ref(&value) {
        return None;
    }
    Some((field.to_string(), value))
}

fn garage_slot_references_to_truck(
    parsed: &super::parser::ParsedTruckSave,
    truck_id_normalized: &str,
) -> Vec<TruckGarageSlotReference> {
    let garage_names = parsed
        .garages
        .iter()
        .map(|garage| {
            (
                normalize_sii_unit_id(&garage.garage_id),
                garage.garage_display_name.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut references = Vec::new();

    for block in parsed
        .unit_blocks
        .values()
        .filter(|block| block.unit_type == "garage")
    {
        let vehicles = extract_array_entries(&block.raw_block, "vehicles")
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let drivers = extract_array_entries(&block.raw_block, "drivers")
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let arrays_have_matching_indices = vehicles
            .iter()
            .filter(|(_, truck_id)| !is_null_ref(truck_id))
            .all(|(index, _)| drivers.contains_key(index))
            && drivers
                .iter()
                .filter(|(_, driver_id)| !is_null_ref(driver_id))
                .all(|(index, _)| vehicles.contains_key(index));

        for (index, vehicle_ref) in vehicles.iter() {
            if normalize_sii_unit_id(vehicle_ref) != truck_id_normalized {
                continue;
            }
            references.push(TruckGarageSlotReference {
                garage_id: block.id.clone(),
                garage_display_name: garage_names
                    .get(&normalize_sii_unit_id(&block.id))
                    .cloned()
                    .flatten(),
                slot_index: *index,
                vehicle_ref: Some(vehicle_ref.clone()),
                driver_ref: drivers
                    .get(index)
                    .filter(|driver_ref| !is_null_ref(driver_ref))
                    .cloned(),
                arrays_have_matching_indices,
            });
        }
    }

    references.sort_by(|left, right| {
        (left.garage_id.as_str(), left.slot_index)
            .cmp(&(right.garage_id.as_str(), right.slot_index))
    });
    references
}

fn is_driver_unit_type(unit_type: &str) -> bool {
    matches!(unit_type, "driver" | "driver_ai" | "driver_player")
}

fn reference_summary(reference: &TruckReferenceMatch) -> String {
    format!(
        "{}:{} {}={}",
        reference.unit_type, reference.unit_id, reference.field_name, reference.value
    )
}

fn annotate_truck_switchability(
    parsed: &super::parser::ParsedTruckSave,
    trucks: &mut [TruckInventoryItem],
) {
    let current_pointer = resolve_current_truck_pointer(parsed).ok();

    for truck in trucks {
        if truck.is_active {
            continue;
        }

        let context = inspect_truck_assignment_context(parsed, &truck.truck_id);
        match context.assignment_kind {
            TruckAssignmentKind::UnassignedOwnedTruck => {
                truck.requires_driver_swap = false;
                truck.is_switchable = true;
                truck.blocked_reason = None;
            }
            TruckAssignmentKind::AiDriverAssigned => {
                truck.requires_driver_swap = true;
                if let Some(driver_id) = context.driver_ref.as_deref() {
                    truck.assigned_driver_id = Some(driver_id.to_string());
                    if truck.driver_display_name.is_none() {
                        truck.driver_display_name = parsed
                            .driver_infos
                            .get(&normalize_sii_unit_id(driver_id))
                            .and_then(|driver| driver.display_name.clone());
                    }
                }
                if current_pointer.is_none() {
                    truck.is_switchable = false;
                    truck.blocked_reason = Some("current_truck_not_found".to_string());
                } else {
                    truck.is_switchable = true;
                    truck.blocked_reason = None;
                }
            }
            TruckAssignmentKind::Ambiguous => {
                truck.is_switchable = false;
                truck.blocked_reason = Some("driver_assignment_ambiguous".to_string());
            }
            TruckAssignmentKind::Unknown => {
                if !context.driver_references.is_empty()
                    || context
                        .garage_references
                        .iter()
                        .any(|reference| reference.driver_ref.is_some())
                    || truck.assigned_driver_id.is_some()
                {
                    truck.requires_driver_swap = true;
                    truck.is_switchable = false;
                    truck.blocked_reason = Some("driver_assignment_unresolved".to_string());
                }
            }
            TruckAssignmentKind::PlayerActive => {}
        }
    }
}

fn find_inventory_item(items: &[TruckInventoryItem], truck_id: &str) -> Option<TruckInventoryItem> {
    items
        .iter()
        .find(|truck| truck.truck_id.eq_ignore_ascii_case(truck_id))
        .cloned()
}

fn set_current_truck_pointer_value(
    content: &str,
    pointer: &CurrentTruckPointer,
    target_truck_id: &str,
) -> Result<(String, bool), String> {
    set_unit_field_value(
        content,
        &pointer.owner_unit_id,
        &pointer.field_name,
        target_truck_id,
    )
}

fn current_truck_pointer_write_error(pointer: &CurrentTruckPointer) -> &'static str {
    match pointer.kind {
        CurrentTruckPointerKind::PlayerMyTruck => "missing_my_truck_pointer",
        CurrentTruckPointerKind::PlayerAssignedVehicles => "missing_assigned_vehicles_pointer",
        CurrentTruckPointerKind::PlayerAssignedTruck => "missing_assigned_truck_pointer",
        CurrentTruckPointerKind::FallbackPlayerVehicles => "current_slot_unresolved",
        CurrentTruckPointerKind::FallbackFirstOwnedTruck => "current_slot_unresolved",
    }
}

fn driver_truck_field_in_content(content: &str, driver_id: &str) -> Option<String> {
    DRIVER_TRUCK_REFERENCE_FIELDS
        .iter()
        .find(|field| unit_field_exists(content, driver_id, field))
        .map(|field| (*field).to_string())
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

    let context = inspect_truck_assignment_context(parsed, &target_truck.truck_id);
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

    match context.assignment_kind {
        TruckAssignmentKind::PlayerActive | TruckAssignmentKind::UnassignedOwnedTruck => {
            return Ok(None);
        }
        TruckAssignmentKind::Ambiguous => {
            if context.unsafe_reason.as_deref() == Some("conflicting_driver_refs") {
                return Err(DriverResolutionError::ConflictingDriverAssignment);
            }
            return Err(DriverResolutionError::AmbiguousDriverAssignment);
        }
        TruckAssignmentKind::Unknown => {
            if context.garage_slot_candidate_count > 1 {
                return Err(DriverResolutionError::AmbiguousGarageDriverRef);
            }
            if !context.driver_references.is_empty()
                || context
                    .garage_references
                    .iter()
                    .any(|reference| reference.driver_ref.is_some())
                || target_truck.assigned_driver_id.is_some()
            {
                return Err(DriverResolutionError::MissingDriverBlock);
            }
            return Ok(None);
        }
        TruckAssignmentKind::AiDriverAssigned => {}
    }

    let Some(driver_id) = context.driver_ref.as_deref() else {
        crate::dev_log!(
            "[truck_change] unresolved driver diagnostics generated target_truck_id={} driver_ref_missing",
            target_truck.truck_id,
        );
        return Err(DriverResolutionError::MissingDriverBlock);
    };
    let Some(driver) = parsed.driver_infos.get(&normalize_sii_unit_id(driver_id)) else {
        return Err(DriverResolutionError::MissingDriverBlock);
    };
    if is_player_driver_ref(parsed, &driver.driver_id) {
        crate::dev_log!(
            "[truck_change] player driver truck reference cannot be used as target AI driver target_truck_id={} driver_id={}",
            target_truck.truck_id,
            driver.driver_id
        );
        return Err(DriverResolutionError::ConflictingDriverAssignment);
    }

    crate::dev_log!(
        "[truck_change] driver resolved by assignment context target_truck_id={} driver_id={} references={} garage_slots={}",
        target_truck.truck_id,
        driver.driver_id,
        context.ai_driver_candidate_count,
        context.garage_slot_candidate_count
    );
    let source = if garage_assignment.is_some() {
        DriverAssignmentSource::ReconciledGarageAndDriver
    } else {
        DriverAssignmentSource::DriverAssignedTruck
    };
    let mut evidence = vec![DriverAssignmentEvidence {
        source: DriverAssignmentSource::DriverAssignedTruck,
        driver_id: Some(driver.driver_id.clone()),
        truck_id: driver.current_truck_id.clone(),
        garage_id: garage_assignment.map(|assignment| assignment.garage_id.clone()),
        slot_index: garage_assignment.map(|assignment| assignment.slot_index),
        detail: driver
            .current_truck_field
            .as_deref()
            .unwrap_or("driver_truck_reference")
            .to_string(),
    }];
    if garage_assignment.is_some() {
        evidence.push(DriverAssignmentEvidence {
            source: DriverAssignmentSource::GarageSlot,
            driver_id: Some(driver.driver_id.clone()),
            truck_id: Some(target_truck.truck_id.clone()),
            garage_id: garage_assignment.map(|assignment| assignment.garage_id.clone()),
            slot_index: garage_assignment.map(|assignment| assignment.slot_index),
            detail: "garage_slot_consistent_with_driver".to_string(),
        });
    }
    Ok(Some(resolved_driver_assignment(
        driver.clone(),
        DriverResolutionKind::FullDriverBlock,
        source,
        garage_assignment,
        evidence,
    )))
}

fn resolved_driver_assignment(
    driver: DriverDisplayInfo,
    resolution_kind: DriverResolutionKind,
    source: DriverAssignmentSource,
    garage_assignment: Option<&super::models::GarageSlotAssignment>,
    evidence: Vec<DriverAssignmentEvidence>,
) -> ResolvedDriverAssignment {
    ResolvedDriverAssignment {
        driver,
        resolution_kind,
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
    resolved: Option<&ResolvedDriverAssignment>,
    error: Option<&DriverResolutionError>,
) -> DriverResolutionDiagnostics {
    let target_truck_id_normalized = normalize_sii_unit_id(&target_truck.truck_id);
    let selected_context = inspect_truck_assignment_context(parsed, &target_truck.truck_id);
    let current_pointer = resolve_current_truck_pointer(parsed).ok();
    let current_truck_id = current_pointer
        .as_ref()
        .map(|pointer| pointer.truck_id.clone());
    let current_context = current_truck_id
        .as_deref()
        .map(|truck_id| inspect_truck_assignment_context(parsed, truck_id));
    let driver_swap_plan_reason = current_truck_id.as_deref().and_then(|truck_id| {
        if resolved.is_some() {
            resolve_driver_swap_garage_plan(parsed, truck_id, &target_truck.truck_id)
                .err()
                .map(|reason| reason.to_string())
        } else {
            None
        }
    });
    let unsafe_reason = error
        .map(|error| error.code().to_string())
        .or(driver_swap_plan_reason)
        .or_else(|| match selected_context.assignment_kind {
            TruckAssignmentKind::Ambiguous | TruckAssignmentKind::Unknown => {
                selected_context.unsafe_reason.clone()
            }
            _ => None,
        });
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
    let garage_driver_ref_unique = garage_driver_id_raw
        .as_deref()
        .map(|driver_id| garage_driver_ref_is_unique(parsed, driver_id, &target_truck.truck_id));
    let checked_sources = driver_resolution_checked_sources(parsed, None);
    let diagnostics = DriverResolutionDiagnostics {
        target_truck_id: target_truck.truck_id.clone(),
        target_truck_id_normalized,
        selected_assignment_kind: Some(selected_context.assignment_kind.clone()),
        current_assignment_kind: current_context
            .as_ref()
            .map(|context| context.assignment_kind.clone()),
        current_truck_id,
        current_garage_id: current_context
            .as_ref()
            .and_then(|context| context.garage_ref.clone()),
        current_garage_slot_index: current_context
            .as_ref()
            .and_then(|context| context.garage_slot_index),
        current_garage_driver_id_raw: current_context.as_ref().and_then(|context| {
            context
                .garage_references
                .first()
                .and_then(|reference| reference.driver_ref.clone())
        }),
        current_driver_ref: current_context
            .as_ref()
            .and_then(|context| context.driver_ref.clone())
            .or_else(|| {
                current_context.as_ref().and_then(|context| {
                    context
                        .garage_references
                        .first()
                        .and_then(|reference| reference.driver_ref.clone())
                })
            }),
        garage_id: selected_context.garage_ref.clone(),
        garage_slot_index: selected_context.garage_slot_index,
        garage_driver_id_raw,
        selected_driver_ref: selected_context.driver_ref.clone(),
        player_vehicles_ref: selected_context
            .player_assigned_vehicles_unit
            .clone()
            .or_else(|| {
                current_context
                    .as_ref()
                    .and_then(|context| context.player_assigned_vehicles_unit.clone())
            }),
        garage_driver_id_normalized,
        resolution_kind: resolved.map(|assignment| assignment.resolution_kind.clone()),
        garage_driver_ref_unique,
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
        checked_sources,
        selected_reverse_references: selected_context
            .reverse_references
            .iter()
            .map(reference_summary)
            .collect(),
        current_reverse_references: current_context
            .as_ref()
            .map(|context| {
                context
                    .reverse_references
                    .iter()
                    .map(reference_summary)
                    .collect()
            })
            .unwrap_or_default(),
        selected_evidence: selected_context.evidence.clone(),
        current_evidence: current_context
            .as_ref()
            .map(|context| context.evidence.clone())
            .unwrap_or_default(),
        unsafe_reason,
        resolution_error: error.map(|error| error.code().to_string()),
    };
    crate::dev_log!(
        "[truck_change] driver diagnostics generated target_truck_id={} current_truck_id={} current_garage_id={} current_slot_index={} garage_id={} slot_index={} garage_driver_id={} resolution_kind={} garage_ref_unique={} recognized_drivers={} drivers_pointing_to_target={} arrays_have_matching_indices={} unsafe_reason={} resolution_error={}",
        diagnostics.target_truck_id,
        diagnostics.current_truck_id.as_deref().unwrap_or(""),
        diagnostics.current_garage_id.as_deref().unwrap_or(""),
        diagnostics
            .current_garage_slot_index
            .map(|index| index.to_string())
            .unwrap_or_default(),
        diagnostics.garage_id.as_deref().unwrap_or(""),
        diagnostics
            .garage_slot_index
            .map(|index| index.to_string())
            .unwrap_or_default(),
        diagnostics.garage_driver_id_raw.as_deref().unwrap_or(""),
        diagnostics
            .resolution_kind
            .as_ref()
            .map(|kind| format!("{:?}", kind))
            .unwrap_or_default(),
        diagnostics
            .garage_driver_ref_unique
            .map(|value| value.to_string())
            .unwrap_or_default(),
        diagnostics.recognized_driver_count,
        diagnostics.drivers_pointing_to_target_truck.len(),
        diagnostics.arrays_have_matching_indices,
        diagnostics.unsafe_reason.as_deref().unwrap_or(""),
        diagnostics.resolution_error.as_deref().unwrap_or("")
    );
    diagnostics
}

fn driver_resolution_checked_sources(
    parsed: &super::parser::ParsedTruckSave,
    current_slot: Option<&ResolvedGarageDriverSlot>,
) -> Vec<String> {
    let mut sources = vec![
        "garage_slot".to_string(),
        "driver_assigned_truck".to_string(),
        "player.trucks[]".to_string(),
        "reverse_reference_index".to_string(),
    ];

    if parsed
        .driver_infos
        .values()
        .any(|driver| driver.unit_type == "driver_ai")
    {
        sources.push("driver_ai".to_string());
    }
    if parsed
        .driver_infos
        .values()
        .any(|driver| driver.unit_type == "driver_player")
    {
        sources.push("driver_player".to_string());
    }
    if parsed
        .current_truck_pointer
        .as_ref()
        .map(|pointer| pointer.kind == CurrentTruckPointerKind::PlayerAssignedVehicles)
        .unwrap_or(false)
    {
        sources.push("player.assigned_vehicles".to_string());
        sources.push("player_vehicles.vehicle".to_string());
    }
    if let Some(slot) = current_slot {
        sources.push(slot.source.to_string());
    }

    sources.sort();
    sources.dedup();
    sources
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

fn resolve_driver_swap_garage_plan(
    parsed: &super::parser::ParsedTruckSave,
    previous_truck_id: &str,
    target_truck_id: &str,
) -> Result<DriverSwapGaragePlan, &'static str> {
    let previous_context = inspect_truck_assignment_context(parsed, previous_truck_id);
    let target_context = inspect_truck_assignment_context(parsed, target_truck_id);

    if target_context.assignment_kind != TruckAssignmentKind::AiDriverAssigned {
        return Err(match target_context.assignment_kind {
            TruckAssignmentKind::Ambiguous => "ambiguous_garage_slots",
            TruckAssignmentKind::Unknown => match target_context.unsafe_reason.as_deref() {
                Some("missing_vehicle_block") => "missing_vehicle_block",
                Some("unknown_assignment") => "unknown_assignment",
                _ => "missing_driver_ref",
            },
            _ => "missing_driver_ref",
        });
    }
    if target_context.ai_driver_candidate_count != 1 {
        return Err("multiple_driver_refs");
    }
    if previous_context.garage_slot_candidate_count > 1
        || target_context.garage_slot_candidate_count > 1
    {
        return Err("ambiguous_garage_slots");
    }

    let target_slot = target_context.garage_references.first().cloned();
    let previous_slot = resolve_previous_driver_slot(parsed, previous_truck_id).ok();

    if previous_slot
        .as_ref()
        .map(|slot| !slot.assignment.arrays_have_matching_indices)
        .unwrap_or(false)
        || target_slot
            .as_ref()
            .map(|slot| !slot.arrays_have_matching_indices)
            .unwrap_or(false)
    {
        return Err("ambiguous_garage_slots");
    }

    if previous_slot
        .as_ref()
        .and_then(|slot| slot.assignment.driver_id.as_deref())
        .map(|driver_ref| !is_player_driver_ref(parsed, driver_ref))
        .unwrap_or(false)
    {
        return Err("current_truck_garage_slot_occupied");
    }

    if previous_slot.is_none() && target_slot.is_none() {
        return Err("missing_garage_slot");
    }

    Ok(DriverSwapGaragePlan {
        target_slot,
        previous_slot,
    })
}

fn garage_slot_matches(left: &GarageSlotAssignment, right: &TruckGarageSlotReference) -> bool {
    normalize_sii_unit_id(&left.garage_id) == normalize_sii_unit_id(&right.garage_id)
        && left.slot_index == right.slot_index
}

fn resolve_previous_driver_slot(
    parsed: &super::parser::ParsedTruckSave,
    current_truck_id: &str,
) -> Result<ResolvedGarageDriverSlot, &'static str> {
    let current_truck_id_normalized = normalize_sii_unit_id(current_truck_id);
    if let Some(assignment) = parsed.garage_assignments.get(&current_truck_id_normalized) {
        return validate_previous_driver_slot(parsed, assignment.clone(), false, "garage_slot");
    }

    resolve_previous_driver_slot_from_assigned_garage(parsed, current_truck_id)
        .ok_or("current_truck_garage_assignment_missing")
        .and_then(|slot| {
            validate_previous_driver_slot(
                parsed,
                slot.assignment,
                slot.write_vehicle_slot,
                slot.source,
            )
        })
}

fn validate_previous_driver_slot(
    parsed: &super::parser::ParsedTruckSave,
    assignment: GarageSlotAssignment,
    write_vehicle_slot: bool,
    source: &'static str,
) -> Result<ResolvedGarageDriverSlot, &'static str> {
    if !assignment.arrays_have_matching_indices {
        return Err("current_truck_garage_arrays_inconsistent");
    }
    if assignment
        .driver_id
        .as_deref()
        .map(|driver_id| !is_player_driver_ref(parsed, driver_id))
        .unwrap_or(false)
    {
        return Err("current_truck_garage_slot_occupied");
    }

    Ok(ResolvedGarageDriverSlot {
        assignment,
        write_vehicle_slot,
        source,
    })
}

fn resolve_previous_driver_slot_from_assigned_garage(
    parsed: &super::parser::ParsedTruckSave,
    current_truck_id: &str,
) -> Option<ResolvedGarageDriverSlot> {
    let graph = parsed.truck_graphs.values().find(|graph| {
        normalize_sii_unit_id(&graph.vehicle_id) == normalize_sii_unit_id(current_truck_id)
    })?;
    let assigned_garage = extract_field_value(&graph.vehicle_block, "assigned_garage")
        .filter(|value| !is_null_ref(value))?;
    let garage_block = parsed.unit_blocks.values().find(|block| {
        block.unit_type == "garage"
            && normalize_sii_unit_id(&block.id) == normalize_sii_unit_id(&assigned_garage)
    })?;
    let vehicles = extract_array_entries(&garage_block.raw_block, "vehicles")
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let drivers = extract_array_entries(&garage_block.raw_block, "drivers")
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let declared_vehicle_count = extract_field_value(&garage_block.raw_block, "vehicles")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let declared_driver_count = extract_field_value(&garage_block.raw_block, "drivers")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let indexed_vehicle_count = vehicles.keys().map(|index| index + 1).max().unwrap_or(0);
    let indexed_driver_count = drivers.keys().map(|index| index + 1).max().unwrap_or(0);
    let garage_vehicle_count = declared_vehicle_count.max(indexed_vehicle_count);
    let garage_driver_count = declared_driver_count.max(indexed_driver_count);
    let arrays_have_matching_indices = vehicles
        .iter()
        .filter(|(_, truck_id)| !is_null_ref(truck_id))
        .all(|(index, _)| drivers.contains_key(index))
        && drivers
            .iter()
            .filter(|(_, driver_id)| !is_null_ref(driver_id))
            .all(|(index, _)| vehicles.contains_key(index));
    if !arrays_have_matching_indices {
        return None;
    }

    for index in 0..garage_vehicle_count {
        let Some(vehicle_value) = vehicles.get(&index) else {
            continue;
        };
        let Some(driver_value) = drivers.get(&index) else {
            continue;
        };
        if !is_null_ref(vehicle_value) || !is_null_ref(driver_value) {
            continue;
        }

        let current_truck_id_normalized = normalize_sii_unit_id(current_truck_id);
        return Some(ResolvedGarageDriverSlot {
            assignment: GarageSlotAssignment {
                garage_id: garage_block.id.clone(),
                garage_display_name: None,
                country_code: None,
                country_display_name: None,
                slot_index: index,
                truck_id: current_truck_id.to_string(),
                truck_id_normalized: current_truck_id_normalized,
                driver_id: None,
                driver_id_normalized: None,
                garage_vehicle_count,
                garage_driver_count,
                arrays_have_matching_indices,
            },
            write_vehicle_slot: true,
            source: "vehicle_assigned_garage_free_slot",
        });
    }

    None
}

fn is_player_driver_ref(parsed: &super::parser::ParsedTruckSave, driver_id: &str) -> bool {
    let driver_id_normalized = normalize_sii_unit_id(driver_id);
    if driver_id_normalized.is_empty() {
        return false;
    }

    if parsed
        .driver_infos
        .get(&driver_id_normalized)
        .map(|driver| driver.unit_type == "driver_player")
        .unwrap_or(false)
    {
        return true;
    }

    parsed
        .player_id
        .as_deref()
        .and_then(|player_id| {
            parsed.unit_blocks.values().find(|block| {
                normalize_sii_unit_id(&block.id) == normalize_sii_unit_id(player_id)
                    && block.unit_type == "player"
            })
        })
        .map(|block| {
            extract_array_values(&block.raw_block, "drivers")
                .iter()
                .any(|value| normalize_sii_unit_id(value) == driver_id_normalized)
        })
        .unwrap_or(false)
}

fn duplicate_driver_or_truck_assignments(parsed: &super::parser::ParsedTruckSave) -> Vec<String> {
    let blocks = parsed.unit_blocks.values().cloned().collect::<Vec<_>>();
    assignment_conflicts_from_blocks(&blocks)
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
        odometer_km: None,
        fuel_relative: None,
        wear: None,
        player_vehicle_slot_id: None,
        player_vehicle_slot_index: None,
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
    use crate::features::truck_change::models::{
        CurrentTruckPointerKind, TruckAssignmentKind, TruckSwitchMode,
    };
    use crate::features::truck_change::parser::{parse_truck_save, resolve_current_truck_pointer};
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
 my_vehicles: 3
 my_vehicles[0]: _nameless.slot.active
 my_vehicles[1]: _nameless.slot.free1
 my_vehicles[2]: _nameless.slot.free2
}
player_job : _nameless.job {
 company_truck: _nameless.truck.active
}
player_vehicles : _nameless.slot.active {
 vehicle: _nameless.truck.active
 trailer: null
}
player_vehicles : _nameless.slot.free1 {
 vehicle: _nameless.truck.free1
 trailer: null
}
player_vehicles : _nameless.slot.free2 {
 vehicle: _nameless.truck.free2
 trailer: null
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

    fn garage_only_fixture() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 assigned_truck: _nameless.truck.a
 my_truck: _nameless.truck.a
 trucks: 2
 trucks[0]: _nameless.truck.a
 trucks[1]: _nameless.truck.b
}
vehicle : _nameless.truck.a {
 accessories: 1
 accessories[0]: _nameless.acc.a
}
vehicle_accessory : _nameless.acc.a {
 data_path: "/def/vehicle/truck/man.tgx/data.sii"
}
vehicle : _nameless.truck.b {
 accessories: 1
 accessories[0]: _nameless.acc.b
}
vehicle_accessory : _nameless.acc.b {
 data_path: "/def/vehicle/truck/scania.s_2016/data.sii"
}
garage : garage.old {
 vehicles: 1
 vehicles[0]: _nameless.truck.a
 drivers: 1
 drivers[0]: null
}
garage : garage.oslo {
 vehicles: 1
 vehicles[0]: _nameless.truck.b
 drivers: 1
 drivers[0]: driver.163
}
}
"#
    }

    fn assigned_vehicles_fixture() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 assigned_vehicles: _nameless.assigned.1
 trucks: 5
 trucks[0]: _nameless.truck.1
 trucks[1]: _nameless.truck.2
 trucks[2]: _nameless.truck.3
 trucks[3]: _nameless.truck.4
 trucks[4]: _nameless.truck.5
 my_vehicles: 5
 my_vehicles[0]: _nameless.assigned.1
 my_vehicles[1]: _nameless.assigned.2
 my_vehicles[2]: _nameless.assigned.3
 my_vehicles[3]: _nameless.assigned.4
 my_vehicles[4]: _nameless.assigned.5
 my_truck: null
 assigned_truck: null
}
player_vehicles : _nameless.assigned.1 {
 vehicle: _nameless.truck.4
 trailer: null
}
player_vehicles : _nameless.assigned.2 {
 vehicle: _nameless.truck.2
 trailer: null
}
player_vehicles : _nameless.assigned.3 {
 vehicle: _nameless.truck.3
 trailer: null
}
player_vehicles : _nameless.assigned.4 {
 vehicle: _nameless.truck.1
 trailer: null
}
player_vehicles : _nameless.assigned.5 {
 vehicle: _nameless.truck.5
 trailer: null
}
vehicle : _nameless.truck.1 {
 accessories: 1
 accessories[0]: _nameless.acc.1
}
vehicle_accessory : _nameless.acc.1 {
 data_path: "/def/vehicle/truck/scania.s_2016/data.sii"
}
vehicle : _nameless.truck.2 {
 accessories: 1
 accessories[0]: _nameless.acc.2
}
vehicle_accessory : _nameless.acc.2 {
 data_path: "/def/vehicle/truck/man.tgx/data.sii"
}
vehicle : _nameless.truck.3 {
 accessories: 1
 accessories[0]: _nameless.acc.3
}
vehicle_accessory : _nameless.acc.3 {
 data_path: "/def/vehicle/truck/volvo.fh16/data.sii"
}
vehicle : _nameless.truck.4 {
 accessories: 1
 accessories[0]: _nameless.acc.4
}
vehicle_accessory : _nameless.acc.4 {
 data_path: "/def/vehicle/truck/daf.xf/data.sii"
}
vehicle : _nameless.truck.5 {
 accessories: 1
 accessories[0]: _nameless.acc.5
}
vehicle_accessory : _nameless.acc.5 {
 data_path: "/def/vehicle/truck/renault.t/data.sii"
}
}
"#
    }

    fn assigned_vehicles_driver_ai_fixture() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 assigned_vehicles: _nameless.assigned.1
 trucks: 2
 trucks[0]: _nameless.truck.a
 trucks[1]: _nameless.truck.b
 drivers: 1
 drivers[0]: driver.94
 my_vehicles: 1
 my_vehicles[0]: _nameless.assigned.1
 my_truck: null
 assigned_truck: null
}
player_vehicles : _nameless.assigned.1 {
 vehicle: _nameless.truck.a
 trailer: null
}
vehicle : _nameless.truck.a {
 accessories: 1
 accessories[0]: _nameless.acc.a
}
vehicle_accessory : _nameless.acc.a {
 data_path: "/def/vehicle/truck/daf.xf/data.sii"
}
vehicle : _nameless.truck.b {
 accessories: 1
 accessories[0]: _nameless.acc.b
}
vehicle_accessory : _nameless.acc.b {
 data_path: "/def/vehicle/truck/man.tgx/data.sii"
}
garage : garage.old {
 vehicles: 1
 vehicles[0]: _nameless.truck.a
 drivers: 1
 drivers[0]: driver.94
}
garage : garage.hamburg {
 vehicles: 1
 vehicles[0]: _nameless.truck.b
 drivers: 1
 drivers[0]: null
}
driver_player : driver.94 {
 profit_log: null
}
driver_ai : driver.1 {
 assigned_truck: _nameless.truck.b
}
}
"#
    }

    #[test]
    fn assignment_context_reports_reverse_refs_for_driver_ai_truck() {
        let parsed = parse_truck_save(assigned_vehicles_driver_ai_fixture());
        let context = super::inspect_truck_assignment_context(&parsed, "_nameless.truck.b");

        assert_eq!(
            context.assignment_kind,
            TruckAssignmentKind::AiDriverAssigned
        );
        assert_eq!(context.driver_ref.as_deref(), Some("driver.1"));
        assert_eq!(context.ai_driver_candidate_count, 1);
        assert_eq!(context.garage_slot_candidate_count, 1);
        assert!(context.reverse_references.iter().any(|reference| {
            reference.unit_type == "driver_ai"
                && reference.unit_id == "driver.1"
                && reference.field_name == "assigned_truck"
                && reference.value == "_nameless.truck.b"
        }));
        assert!(context.garage_references.iter().any(|reference| {
            reference.garage_id == "garage.hamburg"
                && reference.slot_index == 0
                && reference.vehicle_ref.as_deref() == Some("_nameless.truck.b")
        }));
    }

    #[test]
    fn unresolved_driver_assignment_does_not_make_other_trucks_unavailable() {
        let content = fixture()
            .replace("drivers[2]: null", "drivers[2]: driver.missing")
            .replace(
                "assigned_truck: _nameless.truck.free2",
                "assigned_truck: _nameless.truck.other",
            );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let free_truck = list
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.free1")
            .unwrap();
        let unresolved_truck = list
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.free2")
            .unwrap();

        assert!(free_truck.is_switchable);
        assert!(!free_truck.requires_driver_swap);
        assert!(!unresolved_truck.is_switchable);
        assert_eq!(
            unresolved_truck.blocked_reason.as_deref(),
            Some("driver_assignment_unresolved")
        );
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
    fn assigned_vehicles_session_returns_current_truck_and_five_owned_trucks() {
        let cache = TruckChangeSessionCache::default();
        let session = initialize_truck_change_session_from_content(
            "profile-a",
            Path::new("game.sii"),
            assigned_vehicles_fixture(),
            &cache,
        )
        .unwrap();

        assert_eq!(session.current_truck.truck_id, "_nameless.truck.4");
        assert_eq!(session.owned_trucks.len(), 5);
        assert_eq!(
            session
                .diagnostics
                .as_ref()
                .and_then(|diagnostics| diagnostics.current_truck_pointer_kind.clone()),
            Some(CurrentTruckPointerKind::PlayerAssignedVehicles)
        );
        assert_eq!(
            session
                .diagnostics
                .as_ref()
                .and_then(|diagnostics| diagnostics.assigned_vehicles_unit_id.as_deref()),
            Some("_nameless.assigned.1")
        );
    }

    #[test]
    fn assigned_vehicles_preview_allows_free_truck_switch() {
        let list = list_owned_trucks_for_switch_from_content(
            Path::new("game.sii"),
            assigned_vehicles_fixture(),
        );
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            assigned_vehicles_fixture(),
            "_nameless.truck.5",
            &list.file_hash,
        );

        assert_eq!(list.trucks.len(), 5);
        assert_eq!(
            list.diagnostics.current_truck_pointer_kind,
            Some(CurrentTruckPointerKind::PlayerAssignedVehicles)
        );
        assert_eq!(preview.current_truck.truck_id, "_nameless.truck.4");
        assert_eq!(preview.mode, TruckSwitchMode::FreeTruck);
        assert!(preview.can_apply, "{:?}", preview.warnings);
    }

    #[test]
    fn assigned_vehicles_apply_writes_player_vehicle_unit_and_preserves_my_truck() {
        let plan =
            apply_switch_to_content(assigned_vehicles_fixture(), "_nameless.truck.5").unwrap();

        assert_eq!(plan.previous_truck_id, "_nameless.truck.4");
        assert!(plan.content.contains(" my_truck: null"));
        assert!(plan.content.contains(" assigned_truck: null"));
        assert!(plan.content.contains(" vehicle: _nameless.truck.5"));
        assert!(!plan.content.contains(" my_truck: _nameless.truck.5"));

        let parsed = parse_truck_save(&plan.content);
        let pointer = resolve_current_truck_pointer(&parsed).unwrap();
        assert_eq!(
            pointer.kind,
            CurrentTruckPointerKind::PlayerAssignedVehicles
        );
        assert_eq!(pointer.truck_id, "_nameless.truck.5");

        let validation =
            validate_truck_switch_content(&plan.content, "_nameless.truck.5", None, None);
        assert!(validation.success, "{:?}", validation.errors);
        assert_eq!(
            validation.actual_truck_id.as_deref(),
            Some("_nameless.truck.5")
        );
    }

    #[test]
    fn assigned_vehicles_allows_unassigned_owned_target_truck() {
        let content = assigned_vehicles_fixture().replace(
            "player_vehicles : _nameless.assigned.5 {\n vehicle: _nameless.truck.5\n trailer: null\n}",
            "player_vehicles : _nameless.assigned.5 {\n vehicle: null\n trailer: null\n}",
        );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.5",
            &list.file_hash,
        );

        assert!(preview.can_apply, "{:?}", preview.warnings);
        assert_eq!(preview.error_code.as_deref(), None);
        let swap_plan = preview.swap_plan.as_ref().unwrap();
        assert!(swap_plan.target_is_free);
        assert_eq!(swap_plan.target_location.as_deref(), Some("unassigned_owned"));
        assert_eq!(
            swap_plan.old_truck_destination.as_deref(),
            Some("unassigned_owned")
        );
        assert_eq!(
            swap_plan.write_case.as_deref(),
            Some("target_unassigned_owned")
        );
        assert!(swap_plan.can_write_safely);

        let plan = apply_switch_to_content(&content, "_nameless.truck.5").unwrap();
        assert_eq!(plan.previous_truck_id, "_nameless.truck.4");
        assert!(plan.affected_driver_id.is_none());
        assert!(plan.driver_received_truck_id.is_none());
        assert!(plan.content.contains(" trucks[3]: _nameless.truck.4"));
        assert!(plan.content.contains(" vehicle: _nameless.truck.5"));

        let parsed = parse_truck_save(&plan.content);
        assert!(parsed
            .player_vehicle_slots
            .iter()
            .all(|slot| slot.truck_id.as_deref() != Some("_nameless.truck.4")));
        assert!(parsed
            .driver_infos
            .values()
            .all(|driver| driver.current_truck_id.as_deref() != Some("_nameless.truck.4")));
        let validation =
            validate_truck_switch_content(&plan.content, "_nameless.truck.5", None, None);
        assert!(validation.success, "{:?}", validation.errors);
    }

    #[test]
    fn assigned_vehicles_driver_ai_swap_resolves_from_driver_block_with_empty_garage_driver_ref() {
        let content = assigned_vehicles_driver_ai_fixture();
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            content,
            "_nameless.truck.b",
            &list.file_hash,
        );

        let target = list
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.b")
            .unwrap();
        assert!(target.requires_driver_swap);
        assert!(target.is_switchable);
        assert_eq!(target.assigned_driver_id.as_deref(), Some("driver.1"));
        assert_eq!(preview.mode, TruckSwitchMode::DriverSwap);
        assert!(preview.can_apply, "{:?}", preview.warnings);
        assert_eq!(
            preview
                .swap_plan
                .as_ref()
                .and_then(|plan| plan.target_driver_id.as_deref()),
            Some("driver.1")
        );
        assert_eq!(
            preview
                .swap_plan
                .as_ref()
                .and_then(|plan| plan.write_case.as_deref()),
            Some("driver_assigned_truck")
        );
    }

    #[test]
    fn assigned_vehicles_driver_ai_swap_writes_player_vehicle_and_driver_assignment() {
        let plan =
            apply_switch_to_content(assigned_vehicles_driver_ai_fixture(), "_nameless.truck.b")
                .unwrap();

        assert_eq!(plan.previous_truck_id, "_nameless.truck.a");
        assert_eq!(plan.affected_driver_id.as_deref(), Some("driver.1"));
        assert_eq!(
            plan.driver_received_truck_id.as_deref(),
            Some("_nameless.truck.a")
        );
        assert!(plan.content.contains(" vehicle: _nameless.truck.b"));
        assert!(plan.content.contains(" assigned_truck: _nameless.truck.a"));

        let validation = validate_truck_switch_content(
            &plan.content,
            "_nameless.truck.b",
            Some("driver.1"),
            Some("_nameless.truck.a"),
        );
        assert!(validation.success, "{:?}", validation.errors);
    }

    #[test]
    fn driver_player_in_previous_slot_does_not_block_driver_ai_swap() {
        let content = fixture()
            .replace("drivers[0]: null", "drivers[0]: driver.94")
            .replace(
                "driver : driver.1",
                "driver_player : driver.94 {\n profit_log: null\n}\ndriver_ai : driver.1",
            );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.free2",
            &list.file_hash,
        );

        assert!(preview.can_apply, "{:?}", preview.warnings);
        let plan = apply_switch_to_content(&content, "_nameless.truck.free2").unwrap();
        assert_eq!(plan.affected_driver_id.as_deref(), Some("driver.1"));
        assert!(plan
            .content
            .contains(" assigned_truck: _nameless.truck.active"));
    }

    #[test]
    fn assigned_vehicles_driver_ai_swap_works_without_previous_garage_context_when_target_slot_exists(
    ) {
        let content = assigned_vehicles_driver_ai_fixture().replace(
            "garage : garage.old {\n vehicles: 1\n vehicles[0]: _nameless.truck.a\n drivers: 1\n drivers[0]: driver.94\n}\n",
            "",
        );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let target = list
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.b")
            .unwrap();
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.b",
            &list.file_hash,
        );

        assert!(target.requires_driver_swap);
        assert!(target.is_switchable);
        assert!(preview.can_apply, "{:?}", preview.warnings);
        assert_eq!(preview.error_code.as_deref(), None);
        let plan = apply_switch_to_content(&content, "_nameless.truck.b").unwrap();
        assert_eq!(plan.affected_driver_id.as_deref(), Some("driver.1"));
        assert!(plan.content.contains(" assigned_truck: _nameless.truck.a"));
    }

    #[test]
    fn previous_truck_assigned_garage_free_slot_can_receive_driver_truck() {
        let content = assigned_vehicles_driver_ai_fixture()
            .replace(
                "vehicle : _nameless.truck.a {\n accessories: 1",
                "vehicle : _nameless.truck.a {\n assigned_garage: garage.old\n accessories: 1",
            )
            .replace(
                "garage : garage.old {\n vehicles: 1\n vehicles[0]: _nameless.truck.a\n drivers: 1\n drivers[0]: driver.94\n}\n",
                "garage : garage.old {\n vehicles: 1\n vehicles[0]: null\n drivers: 1\n drivers[0]: null\n}\n",
            );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.b",
            &list.file_hash,
        );

        assert!(preview.can_apply, "{:?}", preview.warnings);
        let plan = apply_switch_to_content(&content, "_nameless.truck.b").unwrap();
        assert_eq!(plan.affected_driver_id.as_deref(), Some("driver.1"));
        assert!(plan.content.contains(" assigned_truck: _nameless.truck.a"));
        let validation = validate_truck_switch_content(
            &plan.content,
            "_nameless.truck.b",
            Some("driver.1"),
            Some("_nameless.truck.a"),
        );
        assert!(validation.success, "{:?}", validation.errors);
    }

    #[test]
    fn apply_switch_survives_full_reload_validation() {
        let plan = apply_switch_to_content(fixture(), "_nameless.truck.free1").unwrap();
        let validation =
            validate_truck_switch_content(&plan.content, "_nameless.truck.free1", None, None);
        assert!(validation.success, "{:?}", validation.errors);
        assert!(plan.content.contains(" my_truck: _nameless.truck.free1"));
        assert!(plan
            .content
            .contains(" assigned_truck: _nameless.truck.free1"));
        assert!(plan
            .content
            .contains(" company_truck: _nameless.truck.free1"));
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
        assert!(preview
            .warnings
            .contains(&"save_changed_since_session".to_string()));
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
    fn garage_only_driver_reference_preview_blocks_without_driver_block() {
        let content = garage_only_fixture();
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            content,
            "_nameless.truck.b",
            &list.file_hash,
        );

        assert_eq!(preview.mode, TruckSwitchMode::FreeTruck);
        assert!(!preview.can_apply);
        assert_eq!(
            preview.error_code.as_deref(),
            Some("current_slot_unresolved")
        );
        assert_eq!(preview.affected_driver, None);
    }

    #[test]
    fn garage_only_driver_reference_apply_blocks_without_driver_block() {
        match apply_switch_to_content(garage_only_fixture(), "_nameless.truck.b") {
            Ok(_) => panic!("garage-only driver reference should block apply"),
            Err(error) => assert_eq!(error, "current_slot_unresolved"),
        }
    }

    #[test]
    fn duplicate_garage_only_driver_reference_blocks_preview_and_apply() {
        let content = garage_only_fixture().replace("drivers[0]: null", "drivers[0]: driver.163");
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.b",
            &list.file_hash,
        );

        assert!(!preview.can_apply);
        assert_eq!(
            preview.error_code.as_deref(),
            Some("current_slot_unresolved")
        );
        match apply_switch_to_content(&content, "_nameless.truck.b") {
            Ok(_) => panic!("duplicate garage driver reference should block apply"),
            Err(error) => assert_eq!(error, "current_slot_unresolved"),
        }
    }

    #[test]
    fn duplicate_target_truck_garage_slot_blocks_preview() {
        let content = garage_only_fixture().replace(
            "garage : garage.old {",
            "garage : garage.duplicate {\n vehicles: 1\n vehicles[0]: _nameless.truck.b\n drivers: 1\n drivers[0]: null\n}\ngarage : garage.old {",
        );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.b",
            &list.file_hash,
        );

        assert!(!preview.can_apply);
        assert_eq!(
            preview.error_code.as_deref(),
            Some("current_slot_unresolved")
        );
    }

    #[test]
    fn driver_unit_apply_blocks_when_no_safe_garage_context_exists() {
        let content = garage_only_fixture()
            .replace(
                "garage : garage.oslo {\n vehicles: 1\n vehicles[0]: _nameless.truck.b\n drivers: 1\n drivers[0]: driver.163\n}\n}\n",
                "driver_ai : driver.163 {\n assigned_truck: _nameless.truck.b\n}\n}\n",
            )
            .replace(
                "garage : garage.old {\n vehicles: 1\n vehicles[0]: _nameless.truck.a\n drivers: 1\n drivers[0]: null\n}\n",
                "",
            );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.b",
            &list.file_hash,
        );

        assert!(!preview.can_apply);
        assert_eq!(
            preview.error_code.as_deref(),
            Some("current_slot_unresolved")
        );
        match apply_switch_to_content(&content, "_nameless.truck.b") {
            Ok(_) => panic!("missing previous truck garage slot should block apply"),
            Err(error) => assert_eq!(error, "current_slot_unresolved"),
        }
    }

    #[test]
    fn driver_unit_apply_blocks_when_previous_truck_slot_has_other_driver_and_no_target_slot() {
        let content = garage_only_fixture()
            .replace(
                "garage : garage.oslo {\n vehicles: 1\n vehicles[0]: _nameless.truck.b\n drivers: 1\n drivers[0]: driver.163\n}\n}\n",
                "driver_ai : driver.163 {\n assigned_truck: _nameless.truck.b\n}\n}\n",
            )
            .replace("drivers[0]: null", "drivers[0]: driver.999");
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.b",
            &list.file_hash,
        );

        assert!(!preview.can_apply);
        assert_eq!(
            preview.error_code.as_deref(),
            Some("current_slot_unresolved")
        );
        match apply_switch_to_content(&content, "_nameless.truck.b") {
            Ok(_) => panic!("occupied previous truck slot should block apply"),
            Err(error) => assert_eq!(error, "current_slot_unresolved"),
        }
    }

    #[test]
    fn shifted_garage_indices_block_garage_only_preview() {
        let content = garage_only_fixture().replace(
            "drivers: 1\n drivers[0]: driver.163",
            "drivers: 2\n drivers[0]: driver.163\n drivers[1]: driver.999",
        );
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let preview = preview_active_truck_switch_from_content(
            Path::new("game.sii"),
            &content,
            "_nameless.truck.b",
            &list.file_hash,
        );

        assert!(!preview.can_apply);
        assert_eq!(
            preview.error_code.as_deref(),
            Some("current_slot_unresolved")
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

        assert_eq!(preview.mode, TruckSwitchMode::FreeTruck);
        assert!(preview.can_apply, "{:?}", preview.warnings);
        assert_eq!(
            preview
                .swap_plan
                .as_ref()
                .and_then(|plan| plan.write_case.as_deref()),
            Some("player_vehicle_slot")
        );
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
        let content = fixture();
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

        assert_eq!(preview.mode, TruckSwitchMode::FreeTruck);
        assert!(!preview.can_apply);
        assert_eq!(
            preview.error_code.as_deref(),
            Some("old_truck_destination_missing")
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

        assert!(preview.can_apply, "{:?}", preview.warnings);
    }

    #[test]
    fn similar_driver_id_is_only_reported_as_diagnostic() {
        let content = fixture()
            .replace("drivers[2]: null", "drivers[2]: driver.abc")
            .replace("driver : driver.1", "driver : driver.abd")
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

        assert!(preview.can_apply, "{:?}", preview.warnings);
        assert_eq!(preview.affected_driver, None);
        assert_eq!(preview.error_code, None);
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
        assert!(plan.content.contains(" vehicle: _nameless.truck.free2"));
        assert!(plan.content.contains(" vehicle: _nameless.truck.active"));
        assert!(plan
            .content
            .contains(" assigned_truck: _nameless.truck.active"));
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

        assert!(preview.can_apply, "{:?}", preview.warnings);
        assert!(apply_switch_to_content(&content, "_nameless.truck.free2").is_ok());
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
        assert!(list
            .trucks
            .iter()
            .all(|truck| truck.truck_id != "_nameless.truck.job_market"));
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
