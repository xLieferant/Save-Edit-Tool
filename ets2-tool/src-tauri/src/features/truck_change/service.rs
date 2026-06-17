use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::features::backup::service as backup_service;
use crate::features::vehicles::resolve_active_save_from_snapshot;
use crate::models::trucks::ParsedTruck;
use crate::shared::decrypt::decrypt_cached_with_cache;
use crate::shared::paths::game_sii_from_save;
use crate::shared::sii_parser::parse_trucks_from_sii;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

use super::models::{
    ApplyTruckChangeResult, TruckChangePreview, TruckInventoryItem, TruckSwitchList,
};
use super::parser::{
    extract_field_value, graph_dangling_accessories, parse_truck_save, parse_unit_blocks,
};
use super::validator::validate_truck_switch_content;
use super::writer::{set_unit_field_value, unit_field_exists, write_verified_content};

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
        warnings,
    }
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
    let mut warnings = Vec::new();
    let mut can_apply = true;

    if expected_file_hash != actual_hash {
        warnings.push("save_changed_since_list".to_string());
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
    if target_truck.assigned_driver_id.is_some() && !target_truck.is_active {
        warnings.push("truck_assigned_to_driver".to_string());
        can_apply = false;
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

    TruckChangePreview {
        current_truck,
        target_truck,
        warnings,
        expected_file_hash: actual_hash,
        can_apply,
    }
}

pub fn apply_active_truck_switch_transaction(
    save_path_arg: Option<String>,
    target_truck_id: String,
    expected_file_hash: String,
    profile_state: &AppProfileState,
    profile_cache: &ProfileCache,
    decrypt_cache: &DecryptCache,
) -> Result<ApplyTruckChangeResult, String> {
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    let actual_hash = sha256_hex(content.as_bytes());
    if actual_hash != expected_file_hash {
        return Err("save_changed_since_preview".to_string());
    }

    let parsed_before = parse_truck_save(&content);
    let previous_truck_id = parsed_before
        .active_truck_id
        .clone()
        .ok_or_else(|| "current_truck_not_found".to_string())?;

    let preview = preview_active_truck_switch_from_content(
        &game_path,
        &content,
        &target_truck_id,
        &expected_file_hash,
    );
    if !preview.can_apply {
        return Err(format!("preview_blocked:{}", preview.warnings.join(",")));
    }

    let backup_targets = backup_service::recommended_targets(&game_path);
    let backup = backup_service::create_backup_for_targets(
        profile_state,
        "active truck switch",
        &backup_targets,
    )?;
    let backup_id = backup.backup_id.clone();

    let result = (|| {
        let updated_content = apply_switch_to_content(&content, &target_truck_id)?;
        let temp_validation = validate_truck_switch_content(&updated_content, &target_truck_id);
        if !temp_validation.success {
            return Err(format!(
                "temporary_validation_failed:{}",
                temp_validation.errors.join(",")
            ));
        }

        write_verified_content(&game_path, &updated_content, &backup_id, |candidate| {
            let validation = validate_truck_switch_content(candidate, &target_truck_id);
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
        let reloaded = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
        let _production_trucks: Vec<ParsedTruck> = parse_trucks_from_sii(&reloaded);
        let validation = validate_truck_switch_content(&reloaded, &target_truck_id);
        if !validation.success {
            return Err(format!(
                "post_write_verification_failed:{}",
                validation.errors.join(",")
            ));
        }

        let file_hash_after = sha256_hex(reloaded.as_bytes());
        Ok(ApplyTruckChangeResult {
            success: true,
            backup_id: Some(backup_id.clone()),
            previous_truck_id,
            active_truck_id: target_truck_id.clone(),
            file_hash_before: actual_hash,
            file_hash_after,
            validation,
        })
    })();

    match result {
        Ok(value) => Ok(value),
        Err(error) => {
            let rollback = backup_service::restore_backup(profile_state, &backup_id, true);
            invalidate_after_write(&game_path, profile_cache, decrypt_cache);
            match rollback {
                Ok(_) => Err(format!("{};rollback_restored:{}", error, backup_id)),
                Err(rollback_error) => Err(format!(
                    "{};rollback_failed:{}:{}",
                    error, backup_id, rollback_error
                )),
            }
        }
    }
}

pub fn apply_switch_to_content(content: &str, target_truck_id: &str) -> Result<String, String> {
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
    if target.assigned_driver_id.is_some() && !target.is_active {
        return Err("truck_assigned_to_driver".to_string());
    }
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

    Ok(updated)
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
    let content = decrypt_cached_with_cache(&game_path, decrypt_cache)?;
    Ok(list_owned_trucks_for_switch_from_content(
        &game_path, &content,
    ))
}

pub fn read_switch_preview(
    save_path_arg: Option<String>,
    target_truck_id: String,
    expected_file_hash: String,
    profile_state: &AppProfileState,
    decrypt_cache: &DecryptCache,
) -> Result<TruckChangePreview, String> {
    let game_path = resolve_game_sii_path(save_path_arg, profile_state)?;
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

fn find_inventory_item(items: &[TruckInventoryItem], truck_id: &str) -> Option<TruckInventoryItem> {
    items
        .iter()
        .find(|truck| truck.truck_id.eq_ignore_ascii_case(truck_id))
        .cloned()
}

fn missing_inventory_item(truck_id: &str) -> TruckInventoryItem {
    TruckInventoryItem {
        truck_id: truck_id.to_string(),
        display_index: 0,
        brand: None,
        model: None,
        license_plate: None,
        assigned_garage: None,
        assigned_driver_id: None,
        is_active: false,
        is_switchable: false,
        blocked_reason: Some("truck_not_found".to_string()),
        engine_data_path: None,
        transmission_data_path: None,
        accessory_count: 0,
    }
}

fn inspect_assignment_references(content: &str, target_truck_id: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let parsed = parse_truck_save(content);
    if parsed.garage_assignments.contains_key(target_truck_id) {
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

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_switch_to_content, list_owned_trucks_for_switch_from_content,
        preview_active_truck_switch_from_content,
    };
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
        let updated = apply_switch_to_content(fixture(), "_nameless.truck.free1").unwrap();
        let validation = validate_truck_switch_content(&updated, "_nameless.truck.free1");
        assert!(validation.success, "{:?}", validation.errors);
        assert!(updated.contains(" my_truck: _nameless.truck.free1"));
        assert!(updated.contains(" assigned_truck: _nameless.truck.free1"));
        assert!(updated.contains(" company_truck: _nameless.truck.free1"));
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
                .contains(&"save_changed_since_list".to_string())
        );
    }

    #[test]
    fn driver_truck_cannot_be_selected_in_wip() {
        let content = fixture().replace("drivers[2]: null", "drivers[2]: driver.1");
        let list = list_owned_trucks_for_switch_from_content(Path::new("game.sii"), &content);
        let driver_truck = list
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.free2")
            .unwrap();
        assert!(!driver_truck.is_switchable);
        assert_eq!(
            driver_truck.blocked_reason.as_deref(),
            Some("truck_assigned_to_driver")
        );
    }
}
