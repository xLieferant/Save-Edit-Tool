use std::path::{Path, PathBuf};
use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::features::backup::service as backup_service;
use crate::features::vehicles::resolve_active_save_from_snapshot;
use crate::shared::decrypt::decrypt_cached_with_cache;
use crate::shared::paths::game_sii_from_save;
use crate::shared::user_log;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::cache::{CurrentTrailerCacheEntry, TrailerChangeSessionCache};
use super::graph::trailer_dangling_accessories;
use super::models::{
    ApplyTrailerChangeResult, CurrentTrailerPointer, CurrentTrailerPointerKind,
    PlayerTrailerSlotAssignment, TrailerChangePreview, TrailerChangeSession, TrailerInventoryItem,
    TrailerSwapPreviewDetails, TrailerSwitchList, TrailerSwitchMode,
};
use super::parser::{find_unit_block_by_id, parse_trailer_save, resolve_current_trailer_pointer};
use super::validator::{player_trailers_contains, validate_trailer_switch_content};
use super::writer::{TemporaryRollbackSnapshot, set_unit_field_value, write_verified_content};
use crate::features::truck_change::parser::{extract_field_value, normalize_sii_unit_id};

const TRAILER_CHANGE_FEATURE_STATUS: &str = "WiP/Beta";

pub fn log_trailer_change_frontend_event(
    event: String,
    detail: Option<String>,
) -> Result<(), String> {
    let mut lines = vec![format!("Action: {}", event.trim())];
    if let Some(detail) = detail.filter(|value| !value.trim().is_empty()) {
        lines.push(format!("Technical detail: {}", detail.trim()));
    }
    write_trailer_change_log("info", lines);
    Ok(())
}

pub fn list_owned_trailers_for_switch_from_content(
    save_path: &Path,
    content: &str,
) -> TrailerSwitchList {
    let parsed = parse_trailer_save(content);
    let mut warnings = Vec::new();
    if parsed.trailer_order.is_empty() {
        warnings.push("owned_trailers_missing".to_string());
    }
    if resolve_current_trailer_pointer(&parsed).is_err() {
        warnings.push("active_trailer_not_found".to_string());
    }
    if !parsed
        .diagnostics
        .player_trailer_reference_missing_blocks
        .is_empty()
    {
        warnings.push("player_trailer_reference_missing_block".to_string());
    }

    TrailerSwitchList {
        save_path: save_path.display().to_string(),
        file_hash: sha256_hex(content.as_bytes()),
        active_trailer_id: parsed.active_trailer_id.clone(),
        trailers: parsed.trailers,
        diagnostics: parsed.diagnostics,
        warnings,
    }
}

pub fn initialize_trailer_change_session_from_content(
    profile_id: &str,
    save_path: &Path,
    content: &str,
    session_cache: &TrailerChangeSessionCache,
) -> Result<TrailerChangeSession, String> {
    let file_hash = sha256_hex(content.as_bytes());
    if let Some(entry) = session_cache.get(profile_id, save_path, &file_hash) {
        crate::dev_log!("[trailer_change] current trailer cache hit");
        return Ok(session_from_cache_entry(save_path, file_hash, entry));
    }

    let list = list_owned_trailers_for_switch_from_content(save_path, content);
    if list.trailers.is_empty() {
        return Err("owned_trailers_missing".to_string());
    }
    let current_trailer = list
        .active_trailer_id
        .as_deref()
        .and_then(|active| find_inventory_item(&list.trailers, active))
        .ok_or_else(|| "active_trailer_not_found".to_string())?;
    let session = TrailerChangeSession {
        save_path: save_path.display().to_string(),
        save_hash: list.file_hash.clone(),
        current_trailer,
        owned_trailers: list.trailers,
        diagnostics: Some(list.diagnostics),
        warnings: list.warnings,
    };

    session_cache.store(CurrentTrailerCacheEntry::from_session(
        profile_id.to_string(),
        save_path.to_path_buf(),
        &session,
    ));
    Ok(session)
}

pub fn preview_active_trailer_switch_from_content(
    save_path: &Path,
    content: &str,
    target_trailer_id: &str,
    expected_file_hash: &str,
) -> TrailerChangePreview {
    let parsed = parse_trailer_save(content);
    let actual_hash = sha256_hex(content.as_bytes());
    let current_pointer = resolve_current_trailer_pointer(&parsed).ok();
    let current_trailer = current_pointer
        .as_ref()
        .and_then(|pointer| find_inventory_item(&parsed.trailers, &pointer.trailer_id))
        .unwrap_or_else(|| missing_inventory_item("_missing_current"));
    let target_trailer = find_inventory_item(&parsed.trailers, target_trailer_id)
        .unwrap_or_else(|| missing_inventory_item(target_trailer_id));
    let swap_plan =
        build_trailer_swap_preview_details(&parsed, current_pointer.as_ref(), target_trailer_id);
    let mode = if swap_plan.target_player_vehicle_slot_id.is_some() {
        TrailerSwitchMode::SlotSwap
    } else {
        TrailerSwitchMode::FreeTrailer
    };
    let mut warnings = Vec::new();
    let mut can_apply = true;

    if expected_file_hash != actual_hash {
        warnings.push("save_changed_since_session".to_string());
        can_apply = false;
    }
    if current_pointer.is_none() {
        warnings.push("active_trailer_not_found".to_string());
        can_apply = false;
    }
    if !player_trailers_contains(&parsed, target_trailer_id) {
        warnings.push("target_trailer_not_owned".to_string());
        can_apply = false;
    }
    if current_pointer
        .as_ref()
        .map(|pointer| {
            normalize_sii_unit_id(&pointer.trailer_id) == normalize_sii_unit_id(target_trailer_id)
        })
        .unwrap_or(false)
    {
        warnings.push("target_already_active".to_string());
        can_apply = false;
    }
    if !target_trailer.is_available {
        if let Some(reason) = target_trailer.availability_reason.clone() {
            warnings.push(reason);
        }
        can_apply = false;
    }
    if current_pointer
        .as_ref()
        .map(|pointer| !pointer.writable)
        .unwrap_or(false)
    {
        warnings.push("trailer_assignment_unresolved".to_string());
        can_apply = false;
    }
    match super::parser::find_trailer_block_by_id(&parsed.trailer_blocks, target_trailer_id) {
        Some(block) => {
            let dangling = trailer_dangling_accessories(block, &parsed.unit_ids);
            if !dangling.is_empty() {
                warnings.push("dangling_trailer_references".to_string());
                can_apply = false;
            }
        }
        None => {
            warnings.push("target_trailer_not_found".to_string());
            can_apply = false;
        }
    }
    if current_pointer
        .as_ref()
        .and_then(|pointer| {
            resolve_trailer_switch_write_plan(&parsed, pointer, target_trailer_id).err()
        })
        .is_some()
    {
        warnings.push("trailer_assignment_unresolved".to_string());
        can_apply = false;
    }

    let _ = save_path;
    warnings.sort();
    warnings.dedup();
    let error_code = if can_apply {
        None
    } else {
        Some(preview_error_code(&warnings))
    };
    let safe_to_write = swap_plan.can_write_safely;

    TrailerChangePreview {
        mode,
        current_trailer: current_trailer.clone(),
        target_trailer: target_trailer.clone(),
        selected_trailer: target_trailer,
        warnings,
        error_code,
        diagnostics: Some(parsed.diagnostics),
        swap_plan: Some(swap_plan),
        expected_file_hash: actual_hash,
        safe_to_write,
        can_apply,
    }
}

pub fn apply_active_trailer_switch_transaction(
    save_path_arg: Option<String>,
    target_trailer_id: String,
    expected_file_hash: String,
    create_persistent_backup: bool,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
    trailer_change_cache: &TrailerChangeSessionCache,
) -> Result<ApplyTrailerChangeResult, String> {
    let started_at = Instant::now();
    let profile_id = current_profile_id(profile_state)?;
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
    decrypt_cache.invalidate_path(&game_path);
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    let actual_hash = sha256_hex(content.as_bytes());
    if actual_hash != expected_file_hash {
        return Err("save_changed_since_preview".to_string());
    }

    let parsed_before = parse_trailer_save(&content);
    let current_pointer = resolve_current_trailer_pointer(&parsed_before)?;
    let preview = preview_active_trailer_switch_from_content(
        &game_path,
        &content,
        &target_trailer_id,
        &expected_file_hash,
    );
    if !preview.can_apply {
        return Err(preview
            .error_code
            .unwrap_or_else(|| "preview_blocked".to_string()));
    }

    let mut rollback = TemporaryRollbackSnapshot::create(&game_path)?;
    let backup_result = if create_persistent_backup {
        backup_service::create_backup_for_targets(
            profile_state,
            "change trailer on the road",
            &backup_service::recommended_targets(&game_path),
        )
        .map(Some)
        .map_err(|error| format!("backup_failed:{}", error))?
    } else {
        None
    };

    let apply_plan = apply_switch_to_content(
        &content,
        &parsed_before,
        &current_pointer,
        &target_trailer_id,
    )?;
    let previous_trailer_id = apply_plan.previous_trailer_id.clone();
    let expected_target = target_trailer_id.clone();
    let verify_before_write = |candidate: &str| {
        let validation = validate_trailer_switch_content(candidate, &expected_target);
        if validation.success {
            Ok(())
        } else {
            Err(format!(
                "write_verification_failed:{}",
                validation.errors.join(",")
            ))
        }
    };
    if let Err(error) = write_verified_content(&game_path, &apply_plan.content, verify_before_write)
    {
        let _ = rollback.restore();
        return Err(format!("write_failed:{}", error));
    }

    decrypt_cache.invalidate_path(&game_path);
    let refreshed_content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    let validation = validate_trailer_switch_content(&refreshed_content, &target_trailer_id);
    if !validation.success {
        let _ = rollback.restore();
        return Err("verification_failed".to_string());
    }

    invalidate_after_write(&game_path, profile_cache, decrypt_cache);
    trailer_change_cache.invalidate_save(&profile_id, &game_path);
    let refreshed_session = initialize_trailer_change_session_from_content(
        &profile_id,
        &game_path,
        &refreshed_content,
        trailer_change_cache,
    )?;
    rollback.cleanup()?;
    let file_hash_after = sha256_hex(refreshed_content.as_bytes());
    write_trailer_change_log(
        "info",
        vec![
            "Action: Apply trailer change completed".to_string(),
            format!("Previous trailer: {}", previous_trailer_id),
            format!("Active trailer: {}", target_trailer_id),
            format!("Duration ms: {}", started_at.elapsed().as_millis()),
        ],
    );

    Ok(ApplyTrailerChangeResult {
        success: true,
        backup_id: backup_result
            .as_ref()
            .map(|backup| backup.backup_id.clone()),
        persistent_backup_created: backup_result.is_some(),
        temporary_rollback_used: true,
        temporary_rollback_cleaned: rollback.cleaned(),
        previous_trailer_id,
        active_trailer_id: target_trailer_id,
        file_hash_before: actual_hash,
        file_hash_after,
        validation,
        refreshed_session,
    })
}

struct TrailerSwitchWritePlan {
    current_slot: Option<PlayerTrailerSlotAssignment>,
    target_slot: Option<PlayerTrailerSlotAssignment>,
    old_trailer_destination: String,
    write_case: &'static str,
}

pub struct TrailerApplyPlan {
    pub content: String,
    pub previous_trailer_id: String,
}

pub fn apply_switch_to_content(
    content: &str,
    parsed: &super::parser::ParsedTrailerSave,
    current_pointer: &CurrentTrailerPointer,
    target_trailer_id: &str,
) -> Result<TrailerApplyPlan, String> {
    let switch_plan =
        resolve_trailer_switch_write_plan(parsed, current_pointer, target_trailer_id)?;
    let previous_trailer_id = current_pointer.trailer_id.clone();
    let mut updated = content.to_string();

    if let Some(current_slot) = switch_plan.current_slot.as_ref() {
        let (next, changed_current_slot) = set_unit_field_value(
            &updated,
            &current_slot.slot_id,
            "trailer",
            target_trailer_id,
        )?;
        if !changed_current_slot {
            return Err("trailer_assignment_unresolved".to_string());
        }
        updated = next;
    } else if current_pointer.writable {
        let (next, changed_pointer) = set_unit_field_value(
            &updated,
            &current_pointer.owner_unit_id,
            &current_pointer.field_name,
            target_trailer_id,
        )?;
        if !changed_pointer {
            return Err("trailer_assignment_unresolved".to_string());
        }
        updated = next;
    } else {
        return Err("trailer_assignment_unresolved".to_string());
    }

    if current_pointer.writable
        && !matches!(
            current_pointer.kind,
            CurrentTrailerPointerKind::PlayerAssignedVehicles
                | CurrentTrailerPointerKind::FallbackPlayerVehicles
        )
    {
        let (next, _) = set_unit_field_value(
            &updated,
            &current_pointer.owner_unit_id,
            &current_pointer.field_name,
            target_trailer_id,
        )?;
        updated = next;
    }

    if let Some(player_id) = parsed.player_id.as_deref() {
        if let Some(player_block) =
            find_unit_block_by_id(&parsed.unit_blocks, player_id, Some("player"))
        {
            for field in ["assigned_trailer", "my_trailer"] {
                let Some(value) = extract_field_value(&player_block.raw_block, field) else {
                    continue;
                };
                if normalize_sii_unit_id(&value) != normalize_sii_unit_id(&previous_trailer_id) {
                    continue;
                }
                let (next, changed_field) =
                    set_unit_field_value(&updated, player_id, field, target_trailer_id)?;
                if changed_field {
                    updated = next;
                }
            }
        }
    }

    if let Some(target_slot) = switch_plan.target_slot.as_ref() {
        let (next, changed_target_slot) = set_unit_field_value(
            &updated,
            &target_slot.slot_id,
            "trailer",
            &previous_trailer_id,
        )?;
        if !changed_target_slot {
            return Err("old_trailer_destination_missing".to_string());
        }
        updated = next;
    }

    if let Some(player_id) = parsed.player_id.as_deref() {
        if let Some(current_job_id) = player_current_job(content, player_id) {
            if player_job_company_trailer(content, &current_job_id)
                .map(|trailer| {
                    normalize_sii_unit_id(&trailer) == normalize_sii_unit_id(&previous_trailer_id)
                })
                .unwrap_or(false)
            {
                let (next, _) = set_unit_field_value(
                    &updated,
                    &current_job_id,
                    "company_trailer",
                    target_trailer_id,
                )?;
                updated = next;
            }
        }
    }

    Ok(TrailerApplyPlan {
        content: updated,
        previous_trailer_id,
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
) -> Result<TrailerSwitchList, String> {
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
    decrypt_cache.invalidate_path(&game_path);
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    Ok(list_owned_trailers_for_switch_from_content(
        &game_path, &content,
    ))
}

pub fn read_trailer_change_session(
    save_path_arg: Option<String>,
    profile_state: &AppProfileState,
    decrypt_cache: &DecryptCache,
    session_cache: &TrailerChangeSessionCache,
) -> Result<TrailerChangeSession, String> {
    let profile_id = current_profile_id(profile_state)?;
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
    decrypt_cache.invalidate_path(&game_path);
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    write_trailer_change_log(
        "info",
        vec![
            "Action: Save loaded".to_string(),
            format!("Profile: {}", masked_profile(&profile_id)),
        ],
    );
    initialize_trailer_change_session_from_content(&profile_id, &game_path, &content, session_cache)
}

pub fn read_switch_preview(
    save_path_arg: Option<String>,
    target_trailer_id: String,
    expected_file_hash: String,
    profile_state: &AppProfileState,
    decrypt_cache: &DecryptCache,
) -> Result<TrailerChangePreview, String> {
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
    decrypt_cache.invalidate_path(&game_path);
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    Ok(preview_active_trailer_switch_from_content(
        &game_path,
        &content,
        &target_trailer_id,
        &expected_file_hash,
    ))
}

fn resolve_trailer_switch_write_plan(
    parsed: &super::parser::ParsedTrailerSave,
    current_pointer: &CurrentTrailerPointer,
    target_trailer_id: &str,
) -> Result<TrailerSwitchWritePlan, &'static str> {
    if !current_pointer.writable {
        return Err("trailer_assignment_unresolved");
    }
    let current_slot = current_assignment_slot_for_write(parsed, current_pointer);
    if current_slot.is_none()
        && matches!(
            current_pointer.kind,
            CurrentTrailerPointerKind::PlayerAssignedVehicles
                | CurrentTrailerPointerKind::FallbackPlayerVehicles
        )
    {
        return Err("current_slot_unresolved");
    }
    let target_slot = player_vehicle_slot_for_trailer(parsed, target_trailer_id);
    Ok(TrailerSwitchWritePlan {
        current_slot,
        old_trailer_destination: target_slot
            .as_ref()
            .map(|slot| format!("player_vehicles:{}", slot.slot_id))
            .unwrap_or_else(|| "unassigned_owned".to_string()),
        target_slot,
        write_case: if player_vehicle_slot_for_trailer(parsed, target_trailer_id).is_some() {
            "player_vehicle_slot"
        } else {
            "target_unassigned_owned"
        },
    })
}

fn build_trailer_swap_preview_details(
    parsed: &super::parser::ParsedTrailerSave,
    current_pointer: Option<&CurrentTrailerPointer>,
    target_trailer_id: &str,
) -> TrailerSwapPreviewDetails {
    let target_slot = player_vehicle_slot_for_trailer(parsed, target_trailer_id);
    let plan = current_pointer.and_then(|pointer| {
        resolve_trailer_switch_write_plan(parsed, pointer, target_trailer_id).ok()
    });
    let target_is_free = target_slot.is_none();
    TrailerSwapPreviewDetails {
        current_trailer_id: current_pointer.map(|pointer| pointer.trailer_id.clone()),
        target_trailer_id: target_trailer_id.to_string(),
        target_location: if target_slot.is_some() {
            Some("player_vehicle_slot".to_string())
        } else {
            Some("unassigned_owned".to_string())
        },
        old_trailer_destination: plan
            .as_ref()
            .map(|plan| plan.old_trailer_destination.clone()),
        target_is_free,
        target_player_vehicle_slot_id: target_slot.as_ref().map(|slot| slot.slot_id.clone()),
        target_player_vehicle_slot_index: target_slot.as_ref().and_then(|slot| slot.slot_index),
        write_case: plan.as_ref().map(|plan| plan.write_case.to_string()),
        can_write_safely: plan.is_some(),
    }
}

fn current_assignment_slot_for_write(
    parsed: &super::parser::ParsedTrailerSave,
    pointer: &CurrentTrailerPointer,
) -> Option<PlayerTrailerSlotAssignment> {
    if let Some(assigned_vehicles_id) = pointer.referenced_player_vehicle_unit_id.as_deref() {
        let slot = player_vehicle_slot_by_id(parsed, assigned_vehicles_id)?;
        let contains_current = slot
            .trailer_id_normalized
            .as_deref()
            .map(|trailer_id| trailer_id == normalize_sii_unit_id(&pointer.trailer_id))
            .unwrap_or(false);
        if contains_current {
            return Some(slot);
        }
        return None;
    }
    player_vehicle_slot_for_trailer(parsed, &pointer.trailer_id)
}

fn player_vehicle_slot_for_trailer(
    parsed: &super::parser::ParsedTrailerSave,
    trailer_id: &str,
) -> Option<PlayerTrailerSlotAssignment> {
    parsed
        .player_vehicle_assignments
        .get(&normalize_sii_unit_id(trailer_id))
        .cloned()
}

fn player_vehicle_slot_by_id(
    parsed: &super::parser::ParsedTrailerSave,
    slot_id: &str,
) -> Option<PlayerTrailerSlotAssignment> {
    let normalized = normalize_sii_unit_id(slot_id);
    parsed
        .player_vehicle_slots
        .iter()
        .find(|slot| slot.slot_id_normalized == normalized)
        .cloned()
}

fn player_current_job(content: &str, player_id: &str) -> Option<String> {
    let parsed = parse_trailer_save(content);
    let player_block = find_unit_block_by_id(&parsed.unit_blocks, player_id, Some("player"))?;
    extract_field_value(&player_block.raw_block, "current_job")
        .filter(|value| !crate::features::truck_change::parser::is_null_ref(value))
}

fn player_job_company_trailer(content: &str, job_id: &str) -> Option<String> {
    let parsed = parse_trailer_save(content);
    let job_block = find_unit_block_by_id(&parsed.unit_blocks, job_id, None)?;
    extract_field_value(&job_block.raw_block, "company_trailer")
}

fn find_inventory_item(
    items: &[TrailerInventoryItem],
    trailer_id: &str,
) -> Option<TrailerInventoryItem> {
    let normalized = normalize_sii_unit_id(trailer_id);
    items
        .iter()
        .find(|item| normalize_sii_unit_id(&item.trailer_id) == normalized)
        .cloned()
}

fn missing_inventory_item(trailer_id: &str) -> TrailerInventoryItem {
    TrailerInventoryItem {
        id: trailer_id.to_string(),
        trailer_id: trailer_id.to_string(),
        unit_id: trailer_id.to_string(),
        nameless_id: trailer_id.to_string(),
        display_index: 0,
        display_name: "Unknown trailer".to_string(),
        brand: None,
        model: None,
        raw_license_plate: None,
        display_license_plate: None,
        license_plate: None,
        garage_city: None,
        garage_country: None,
        garage_id: None,
        garage_display_name: None,
        assigned_garage: None,
        driver_label: None,
        owner_label: None,
        assignment_label: None,
        is_active: false,
        is_available: false,
        is_switchable: false,
        availability_reason: Some("target_trailer_not_found".to_string()),
        assigned_driver_id: None,
        assigned_storage_id: None,
        source: "missing".to_string(),
        accessory_count: 0,
        cargo_mass: None,
        wear: None,
        player_vehicle_slot_id: None,
        player_vehicle_slot_index: None,
        technical_details: serde_json::json!({ "missing": true }),
    }
}

fn preview_error_code(warnings: &[String]) -> String {
    const PRIORITY: &[&str] = &[
        "save_changed_since_session",
        "owned_trailers_missing",
        "active_trailer_not_found",
        "target_trailer_not_found",
        "target_trailer_not_owned",
        "target_already_active",
        "target_trailer_not_available",
        "trailer_assignment_unresolved",
        "trailer_storage_unresolved",
        "write_verification_failed",
        "backup_failed",
    ];
    PRIORITY
        .iter()
        .find(|code| warnings.iter().any(|warning| warning == *code))
        .map(|code| (*code).to_string())
        .or_else(|| warnings.first().cloned())
        .unwrap_or_else(|| "preview_blocked".to_string())
}

fn session_from_cache_entry(
    save_path: &Path,
    save_hash: String,
    entry: CurrentTrailerCacheEntry,
) -> TrailerChangeSession {
    TrailerChangeSession {
        save_path: save_path.display().to_string(),
        save_hash,
        current_trailer: entry.trailer,
        owned_trailers: entry.owned_trailers,
        diagnostics: entry.diagnostics,
        warnings: Vec::new(),
    }
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

fn masked_profile(profile_id: &str) -> String {
    Path::new(profile_id)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<masked profile>".to_string())
}

fn write_trailer_change_log(level: &str, lines: Vec<String>) {
    let mut body = Vec::new();
    body.push("[TrailerChange] ==================================================".to_string());
    body.push(format!(
        "[TrailerChange] App version: {}",
        env!("CARGO_PKG_VERSION")
    ));
    body.push(format!(
        "[TrailerChange] Feature status: {}",
        TRAILER_CHANGE_FEATURE_STATUS
    ));
    body.extend(
        lines
            .into_iter()
            .map(|line| format!("[TrailerChange] {}", line)),
    );
    body.push("[TrailerChange] ==================================================".to_string());
    let message = body.join("\n");
    let result = match level {
        "error" => user_log::user_log_error("TrailerChange", message),
        "warn" => user_log::user_log_warn("TrailerChange", message),
        _ => user_log::user_log_info("TrailerChange", message),
    };
    if let Err(error) = result {
        crate::dev_log!("[trailer_change] user log write failed: {}", error);
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
