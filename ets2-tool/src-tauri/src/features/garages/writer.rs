use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::features::ets2save::sii_codec::replace_file_atomic;
use crate::features::truck_change::parser::{
    UnitBlock, extract_array_entries, extract_field_value, is_null_ref, parse_unit_blocks,
};

use super::parser::city_token_from_garage_id;

const LINE_FEED: char = 10 as char;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GarageWritePlan {
    pub content: String,
    pub changed_unit_ids: Vec<String>,
}

pub fn apply_garage_changes(
    content: &str,
    garage_id: &str,
    target_status_and_capacity: Option<(i32, usize)>,
    set_as_headquarters: bool,
) -> Result<GarageWritePlan, String> {
    let mut updated = content.to_string();
    let mut changed_unit_ids = Vec::new();

    if let Some((target_status, target_capacity)) = target_status_and_capacity {
        updated = resize_garage_capacity(&updated, garage_id, target_status, target_capacity)?;
        changed_unit_ids.push(garage_id.to_string());
    }

    if set_as_headquarters {
        let (next_content, player_id) = set_headquarters(&updated, garage_id)?;
        updated = next_content;
        changed_unit_ids.push(player_id);
    }

    changed_unit_ids.sort();
    changed_unit_ids.dedup();
    Ok(GarageWritePlan {
        content: updated,
        changed_unit_ids,
    })
}

pub fn write_verified_content(
    target_path: &Path,
    content: &str,
    verify_candidate: impl Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    verify_candidate(content)?;
    let temporary_path = temporary_path_for(target_path);

    let write_result = (|| {
        let mut temporary_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|_| "save_write_failed:temporary_create".to_string())?;
        temporary_file
            .write_all(content.as_bytes())
            .map_err(|_| "save_write_failed:temporary_write".to_string())?;
        temporary_file
            .flush()
            .map_err(|_| "save_write_failed:temporary_flush".to_string())?;
        temporary_file
            .sync_all()
            .map_err(|_| "save_write_failed:temporary_sync".to_string())?;
        drop(temporary_file);
        let temporary_content = fs::read_to_string(&temporary_path)
            .map_err(|_| "save_write_failed:temporary_readback".to_string())?;
        verify_candidate(&temporary_content)?;
        replace_file_atomic(&temporary_path, target_path)
            .map_err(|_| "save_write_failed:atomic_replace".to_string())
    })();

    if write_result.is_err() && temporary_path.exists() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

fn resize_garage_capacity(
    content: &str,
    garage_id: &str,
    target_status: i32,
    target_capacity: usize,
) -> Result<String, String> {
    let block = unique_unit_block(content, "garage", garage_id)?;
    validate_reusable_profit_log(content, &block)?;
    let with_vehicles = resize_array_field(&block.raw_block, "vehicles", target_capacity)?;
    let with_drivers = resize_array_field(&with_vehicles, "drivers", target_capacity)?;
    let rewritten_block =
        replace_scalar_field(&with_drivers, "status", &target_status.to_string())?;
    replace_unit_block(content, &block, &rewritten_block)
}

fn validate_reusable_profit_log(content: &str, garage_block: &UnitBlock) -> Result<(), String> {
    let profit_log_id = extract_field_value(&garage_block.raw_block, "profit_log")
        .filter(|value| !is_null_ref(value))
        .ok_or_else(|| format!("garage_profit_log_reference_unresolved:{}", garage_block.id))?;
    let matching = parse_unit_blocks(content)
        .into_iter()
        .filter(|block| block.id == profit_log_id)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [block] if block.unit_type == "profit_log" => Ok(()),
        [] | [_] => Err(format!(
            "garage_profit_log_reference_unresolved:{}",
            garage_block.id
        )),
        _ => Err(format!(
            "garage_profit_log_reference_ambiguous:{}",
            garage_block.id
        )),
    }
}

fn set_headquarters(content: &str, garage_id: &str) -> Result<(String, String), String> {
    let city_token = city_token_from_garage_id(garage_id)
        .ok_or_else(|| "garage_block_invalid:invalid_garage_id".to_string())?;
    let economy_blocks = parse_unit_blocks(content)
        .into_iter()
        .filter(|block| block.unit_type == "economy")
        .collect::<Vec<_>>();
    let economy_block = match economy_blocks.as_slice() {
        [] => return Err("garage_block_invalid:economy_missing".to_string()),
        [block] => block,
        _ => return Err("garage_reference_ambiguous:economy".to_string()),
    };
    let player_id = extract_field_value(&economy_block.raw_block, "player")
        .ok_or_else(|| "garage_block_invalid:player_reference_missing".to_string())?;
    let player_block = unique_unit_block(content, "player", &player_id)?;
    let rewritten_block = replace_scalar_field(&player_block.raw_block, "hq_city", &city_token)?;
    Ok((
        replace_unit_block(content, &player_block, &rewritten_block)?,
        player_id,
    ))
}

fn unique_unit_block(content: &str, unit_type: &str, unit_id: &str) -> Result<UnitBlock, String> {
    let matching = parse_unit_blocks(content)
        .into_iter()
        .filter(|block| block.unit_type == unit_type && block.id == unit_id)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [] if unit_type == "garage" => Err(format!("garage_not_found:{unit_id}")),
        [] => Err(format!("garage_block_invalid:{unit_type}_missing")),
        [block] => Ok(block.clone()),
        _ => Err(format!("garage_reference_ambiguous:{unit_id}")),
    }
}

fn resize_array_field(raw_block: &str, field: &str, target_count: usize) -> Result<String, String> {
    let mut lines = raw_block.lines().map(str::to_string).collect::<Vec<_>>();
    let scalar_prefix = format!("{field}:");
    let count_lines = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(&scalar_prefix))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let count_index = match count_lines.as_slice() {
        [index] => *index,
        [] => return Err(format!("garage_block_invalid:{field}_missing")),
        _ => return Err(format!("garage_block_invalid:{field}_ambiguous")),
    };
    let current_count = lines[count_index]
        .trim()
        .split_once(':')
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .ok_or_else(|| format!("garage_block_invalid:{field}_invalid"))?;
    let entries = extract_array_entries(raw_block, field);
    let entry_indices = entries
        .iter()
        .map(|(index, _)| *index)
        .collect::<HashSet<_>>();
    if entries.len() != current_count
        || entry_indices.len() != current_count
        || (0..current_count).any(|index| !entry_indices.contains(&index))
    {
        return Err(format!("garage_block_invalid:{field}_indices_invalid"));
    }

    let count_indent = line_indent(&lines[count_index]);
    lines[count_index] = format!("{count_indent}{field}: {target_count}");
    if target_count == current_count {
        return Ok(join_lines(&lines));
    }
    if target_count < current_count {
        if let Some((index, _)) = entries
            .iter()
            .find(|(index, value)| *index >= target_count && !is_null_ref(value))
        {
            return Err(format!(
                "garage_downgrade_capacity_exceeded:{field}:slot={index}"
            ));
        }
        lines.retain(|line| match array_index_for_line(line, field) {
            Some(index) => index < target_count,
            None => true,
        });
        return Ok(join_lines(&lines));
    }

    let array_prefix = format!("{field}[");
    let last_array_index = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(&array_prefix))
        .map(|(index, _)| index)
        .next_back();
    let insert_at = last_array_index.map_or(count_index + 1, |index| index + 1);
    let array_indent = last_array_index
        .map(|index| line_indent(&lines[index]))
        .unwrap_or_else(|| count_indent.clone());
    let new_entries = (current_count..target_count)
        .map(|index| format!("{array_indent}{field}[{index}]: null"))
        .collect::<Vec<_>>();
    lines.splice(insert_at..insert_at, new_entries);
    Ok(join_lines(&lines))
}

fn array_index_for_line(line: &str, field: &str) -> Option<usize> {
    let prefix = format!("{field}[");
    let suffix = line.trim_start().strip_prefix(&prefix)?;
    let (index, _) = suffix.split_once("]:")?;
    index.parse::<usize>().ok()
}

fn replace_scalar_field(raw_block: &str, field: &str, value: &str) -> Result<String, String> {
    let mut lines = raw_block.lines().map(str::to_string).collect::<Vec<_>>();
    let prefix = format!("{field}:");
    let matching = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(&prefix))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let index = match matching.as_slice() {
        [index] => *index,
        [] => return Err(format!("garage_block_invalid:{field}_missing")),
        _ => return Err(format!("garage_block_invalid:{field}_ambiguous")),
    };
    let indent = line_indent(&lines[index]);
    lines[index] = format!("{indent}{field}: {value}");
    Ok(join_lines(&lines))
}

fn replace_unit_block(
    content: &str,
    block: &UnitBlock,
    rewritten_block: &str,
) -> Result<String, String> {
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    if block.end_line >= lines.len() || block.start_line > block.end_line {
        return Err("garage_block_invalid:block_range".to_string());
    }
    lines.splice(
        block.start_line..=block.end_line,
        rewritten_block.lines().map(str::to_string),
    );
    let mut updated = join_lines(&lines);
    if content.ends_with(LINE_FEED) {
        updated.push(LINE_FEED);
    }
    Ok(updated)
}

fn join_lines(lines: &[String]) -> String {
    lines.join(&LINE_FEED.to_string())
}

fn line_indent(line: &str) -> String {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .collect()
}

fn temporary_path_for(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("game.sii");
    target_path.with_file_name(format!("{file_name}.garage.{}.tmp", Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{apply_garage_changes, write_verified_content};
    use crate::features::garages::models::{GarageOperation, GarageSize};
    use crate::features::garages::parser::parse_garages_from_sii;
    use crate::features::garages::validator::{GarageVerificationSpec, verify_garage_mutation};
    use crate::features::truck_change::parser::parse_unit_blocks;
    use crate::shared::ets2data::validate::sha256_hex_bytes;
    use uuid::Uuid;

    const SAMPLE: &str = include_str!("../../../test-fixtures/garages/garage_samples.sii");
    const REAL_SAMPLE: &str = include_str!("../../../test-fixtures/decrypt/plain_game.sii");

    #[test]
    fn purchase_expands_only_the_selected_garage() {
        let plan = apply_garage_changes(SAMPLE, "garage.los_angeles", Some((3, 5)), false).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let purchased = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.los_angeles")
            .unwrap();
        assert_eq!(purchased.status, Some(3));
        assert_eq!(purchased.vehicle_slot_count, 5);
        assert_eq!(purchased.driver_slot_count, 5);
        assert_eq!(purchased.trailer_slot_count, 0);
        assert!(
            purchased
                .slots
                .iter()
                .all(|slot| { slot.truck_id.is_none() && slot.driver_id.is_none() })
        );
        assert_eq!(
            purchased.profit_log_id.as_deref(),
            Some("profit.los_angeles")
        );
        assert!(
            plan.content
                .contains("future_garage_field: preserved_by_reader")
        );
    }

    #[test]
    fn upgrade_preserves_existing_references() {
        let small = SAMPLE
            .replace("vehicles[0]: null", "vehicles[0]: truck.one")
            .replace("drivers[0]: null", "drivers[0]: driver.one");
        let plan = apply_garage_changes(&small, "garage.paris", Some((3, 5)), false).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let upgraded = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap();
        assert_eq!(upgraded.slots[0].truck_id.as_deref(), Some("truck.one"));
        assert_eq!(upgraded.slots[0].driver_id.as_deref(), Some("driver.one"));
        assert_eq!(upgraded.vehicle_slot_count, 5);
        assert_eq!(upgraded.driver_slot_count, 5);
    }

    #[test]
    fn setting_headquarters_changes_only_player_hq_city() {
        let plan = apply_garage_changes(SAMPLE, "garage.paris", None, true).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        assert_eq!(
            parsed.headquarters_garage_id.as_deref(),
            Some("garage.paris")
        );
        assert_eq!(plan.changed_unit_ids, vec!["_player"]);
    }

    #[test]
    fn safe_shrink_preserves_assignments_and_metadata() {
        let plan = apply_garage_changes(SAMPLE, "garage.berlin", Some((2, 3)), false).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let garage = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();

        assert_eq!(garage.status, Some(2));
        assert_eq!(garage.vehicle_slot_count, 3);
        assert_eq!(garage.driver_slot_count, 3);
        assert_eq!(garage.slots[0].truck_id.as_deref(), Some("truck.one"));
        assert_eq!(garage.slots[1].truck_id.as_deref(), Some("truck.two"));
        assert_eq!(garage.slots[0].driver_id.as_deref(), Some("driver.one"));
        assert_eq!(garage.trailer_ids, vec!["trailer.one"]);
        assert!(
            plan.content
                .contains("future_garage_field: preserved_by_reader")
        );
    }

    #[test]
    fn shrink_rejects_reference_outside_target_capacity() {
        let occupied = SAMPLE.replace("vehicles[4]: null", "vehicles[4]: truck.five");
        let error =
            apply_garage_changes(&occupied, "garage.berlin", Some((2, 3)), false).unwrap_err();
        assert_eq!(error, "garage_downgrade_capacity_exceeded:vehicles:slot=4");
    }

    #[test]
    fn shrink_rejects_driver_outside_target_capacity() {
        let occupied = SAMPLE.replace("drivers[4]: null", "drivers[4]: driver.five");
        let error =
            apply_garage_changes(&occupied, "garage.berlin", Some((2, 3)), false).unwrap_err();
        assert_eq!(error, "garage_downgrade_capacity_exceeded:drivers:slot=4");
    }

    #[test]
    fn mutation_rejects_unknown_garage_id() {
        let error =
            apply_garage_changes(SAMPLE, "garage.missing", Some((2, 3)), false).unwrap_err();
        assert_eq!(error, "garage_not_found:garage.missing");
    }

    #[test]
    fn purchase_rejects_missing_profit_log_block() {
        let invalid = SAMPLE.replace(
            "profit_log : profit.los_angeles",
            "profit_log : profit.detached",
        );
        let error =
            apply_garage_changes(&invalid, "garage.los_angeles", Some((3, 5)), false).unwrap_err();
        assert_eq!(
            error,
            "garage_profit_log_reference_unresolved:garage.los_angeles"
        );
    }

    #[test]
    fn failed_preverification_keeps_original_file_unchanged() {
        let path =
            std::env::temp_dir().join(format!("ets2-garage-writer-test-{}.sii", Uuid::new_v4()));
        fs::write(&path, SAMPLE).unwrap();

        let error = write_verified_content(&path, "invalid candidate", |_| {
            Err("save_verification_failed:test".to_string())
        })
        .unwrap_err();

        assert_eq!(error, "save_verification_failed:test");
        assert_eq!(fs::read_to_string(&path).unwrap(), SAMPLE);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn atomic_write_roundtrip_verifies_anonymized_purchase() {
        let path = std::env::temp_dir().join(format!(
            "ets2-garage-write-roundtrip-{}.sii",
            Uuid::new_v4()
        ));
        fs::write(&path, SAMPLE).unwrap();
        let plan = apply_garage_changes(SAMPLE, "garage.los_angeles", Some((3, 5)), false).unwrap();
        let spec = GarageVerificationSpec {
            operation: GarageOperation::Purchase,
            target_size: Some(GarageSize::Large),
            set_as_headquarters: false,
        };

        write_verified_content(&path, &plan.content, |candidate| {
            verify_garage_mutation(SAMPLE, candidate, "garage.los_angeles", &spec).map(|_| ())
        })
        .unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let verified =
            verify_garage_mutation(SAMPLE, &written, "garage.los_angeles", &spec).unwrap();
        assert_eq!(verified.updated_state.size, GarageSize::Large);
        assert_eq!(verified.updated_state.vehicle_slot_count, 5);
        assert_eq!(verified.updated_state.driver_slot_count, 5);
        assert!(
            verified
                .updated_state
                .slots
                .iter()
                .all(|slot| { slot.truck_id.is_none() && slot.driver_id.is_none() })
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn atomic_write_roundtrip_verifies_upgrade_and_downgrade() {
        let path = std::env::temp_dir().join(format!(
            "ets2-garage-resize-roundtrip-{}.sii",
            Uuid::new_v4()
        ));
        fs::write(&path, SAMPLE).unwrap();

        let upgrade_plan =
            apply_garage_changes(SAMPLE, "garage.paris", Some((3, 5)), false).unwrap();
        let upgrade_spec = GarageVerificationSpec {
            operation: GarageOperation::Upgrade,
            target_size: Some(GarageSize::Large),
            set_as_headquarters: false,
        };
        write_verified_content(&path, &upgrade_plan.content, |candidate| {
            verify_garage_mutation(SAMPLE, candidate, "garage.paris", &upgrade_spec).map(|_| ())
        })
        .unwrap();

        let upgraded = fs::read_to_string(&path).unwrap();
        let verified_upgrade =
            verify_garage_mutation(SAMPLE, &upgraded, "garage.paris", &upgrade_spec).unwrap();
        assert_eq!(verified_upgrade.updated_state.size, GarageSize::Large);

        let downgrade_plan =
            apply_garage_changes(&upgraded, "garage.paris", Some((2, 3)), false).unwrap();
        let downgrade_spec = GarageVerificationSpec {
            operation: GarageOperation::Update,
            target_size: Some(GarageSize::Small),
            set_as_headquarters: false,
        };
        write_verified_content(&path, &downgrade_plan.content, |candidate| {
            verify_garage_mutation(&upgraded, candidate, "garage.paris", &downgrade_spec)
                .map(|_| ())
        })
        .unwrap();

        let downgraded = fs::read_to_string(&path).unwrap();
        let verified_downgrade =
            verify_garage_mutation(&upgraded, &downgraded, "garage.paris", &downgrade_spec)
                .unwrap();
        assert_eq!(verified_downgrade.updated_state.size, GarageSize::Small);
        assert_eq!(verified_downgrade.updated_state.vehicle_slot_count, 3);
        assert_eq!(verified_downgrade.updated_state.driver_slot_count, 3);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sequential_purchases_use_the_reloaded_content_and_create_no_trucks() {
        let mut content = SAMPLE.replace(" garages: 4", " garages: 5").replace(
            " garages[3]: garage.unknown_city",
            " garages[3]: garage.unknown_city\n garages[4]: garage.madrid",
        );
        let insert_at = content.rfind("\n}").unwrap();
        content.insert_str(
            insert_at,
            "\n\ngarage : garage.madrid {\n vehicles: 0\n drivers: 0\n trailers: 0\n status: 0\n profit_log: profit.madrid\n productivity: 0\n}\n\nprofit_log : profit.madrid {\n}",
        );
        let vehicle_blocks_before = parse_unit_blocks(&content)
            .iter()
            .filter(|block| block.unit_type == "vehicle")
            .count();

        let first =
            apply_garage_changes(&content, "garage.los_angeles", Some((3, 5)), false).unwrap();
        let first_hash = sha256_hex_bytes(first.content.as_bytes());
        let second =
            apply_garage_changes(&first.content, "garage.madrid", Some((3, 5)), false).unwrap();
        let second_hash = sha256_hex_bytes(second.content.as_bytes());
        let parsed = parse_garages_from_sii(&second.content).unwrap();

        for garage_id in ["garage.los_angeles", "garage.madrid"] {
            let garage = parsed
                .garages
                .iter()
                .find(|garage| garage.garage_id == garage_id)
                .unwrap();
            assert_eq!(garage.status, Some(3));
            assert_eq!(garage.vehicle_slot_count, 5);
            assert_eq!(garage.driver_slot_count, 5);
            assert_eq!(garage.assigned_truck_count, 0);
            assert_eq!(garage.assigned_driver_count, 0);
        }
        assert_ne!(first_hash, second_hash);
        assert_eq!(
            parse_unit_blocks(&second.content)
                .iter()
                .filter(|block| block.unit_type == "vehicle")
                .count(),
            vehicle_blocks_before
        );
    }

    #[test]
    fn three_real_fixture_purchases_reload_disk_and_create_no_units() {
        let path = std::env::temp_dir().join(format!(
            "ets2-garage-three-purchases-{}.sii",
            Uuid::new_v4()
        ));
        fs::write(&path, REAL_SAMPLE).unwrap();
        let unit_counts_before = save_unit_counts(REAL_SAMPLE);
        let mut previous_hash = sha256_hex_bytes(REAL_SAMPLE.as_bytes());
        let purchased_ids = ["garage.leipzig", "garage.cardiff", "garage.magdeburg"];

        for garage_id in purchased_ids {
            let before = fs::read_to_string(&path).unwrap();
            assert_eq!(sha256_hex_bytes(before.as_bytes()), previous_hash);
            let plan = apply_garage_changes(&before, garage_id, Some((3, 5)), false).unwrap();
            let spec = GarageVerificationSpec {
                operation: GarageOperation::Purchase,
                target_size: Some(GarageSize::Large),
                set_as_headquarters: false,
            };
            write_verified_content(&path, &plan.content, |candidate| {
                verify_garage_mutation(&before, candidate, garage_id, &spec).map(|_| ())
            })
            .unwrap();

            let after = fs::read_to_string(&path).unwrap();
            let verified = verify_garage_mutation(&before, &after, garage_id, &spec).unwrap();
            assert_eq!(verified.updated_state.status, Some(3));
            assert_eq!(verified.updated_state.vehicle_slot_count, 5);
            assert_eq!(verified.updated_state.driver_slot_count, 5);
            assert_eq!(verified.updated_state.assigned_truck_count, 0);
            assert_eq!(verified.updated_state.assigned_driver_count, 0);
            previous_hash = sha256_hex_bytes(after.as_bytes());
        }

        let final_content = fs::read_to_string(&path).unwrap();
        let parsed = parse_garages_from_sii(&final_content).unwrap();
        for garage_id in purchased_ids {
            let garage = parsed
                .garages
                .iter()
                .find(|garage| garage.garage_id == garage_id)
                .unwrap();
            assert_eq!(garage.status, Some(3));
            assert_eq!(garage.vehicle_slot_count, 5);
            assert_eq!(garage.driver_slot_count, 5);
        }
        assert_eq!(save_unit_counts(&final_content), unit_counts_before);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn real_fixture_upgrade_and_downgrade_preserve_existing_slots() {
        let parsed = parse_garages_from_sii(REAL_SAMPLE).unwrap();
        let previous = parsed
            .garages
            .iter()
            .find(|garage| garage.size == GarageSize::Small)
            .unwrap();
        let garage_id = previous.garage_id.clone();

        let upgrade_plan =
            apply_garage_changes(REAL_SAMPLE, &garage_id, Some((3, 5)), false).unwrap();
        let upgrade_spec = GarageVerificationSpec {
            operation: GarageOperation::Upgrade,
            target_size: Some(GarageSize::Large),
            set_as_headquarters: false,
        };
        let upgraded = verify_garage_mutation(
            REAL_SAMPLE,
            &upgrade_plan.content,
            &garage_id,
            &upgrade_spec,
        )
        .unwrap();
        assert_eq!(&upgraded.updated_state.slots[..3], &previous.slots[..3]);
        assert!(upgraded.updated_state.slots[3].truck_id.is_none());
        assert!(upgraded.updated_state.slots[3].driver_id.is_none());
        assert!(upgraded.updated_state.slots[4].truck_id.is_none());
        assert!(upgraded.updated_state.slots[4].driver_id.is_none());

        let downgrade_plan =
            apply_garage_changes(&upgrade_plan.content, &garage_id, Some((2, 3)), false).unwrap();
        let downgrade_spec = GarageVerificationSpec {
            operation: GarageOperation::Update,
            target_size: Some(GarageSize::Small),
            set_as_headquarters: false,
        };
        let downgraded = verify_garage_mutation(
            &upgrade_plan.content,
            &downgrade_plan.content,
            &garage_id,
            &downgrade_spec,
        )
        .unwrap();
        assert_eq!(downgraded.updated_state.slots, previous.slots);
    }

    #[test]
    fn real_fixture_blocks_downgrade_with_occupied_removed_slot() {
        let error =
            apply_garage_changes(REAL_SAMPLE, "garage.lille", Some((2, 3)), false).unwrap_err();
        assert!(error.starts_with("garage_downgrade_capacity_exceeded:vehicles:slot="));
    }

    fn save_unit_counts(content: &str) -> (usize, usize, usize, usize) {
        let blocks = parse_unit_blocks(content);
        (
            blocks
                .iter()
                .filter(|block| block.unit_type == "vehicle")
                .count(),
            blocks
                .iter()
                .filter(|block| block.unit_type == "driver_ai")
                .count(),
            blocks
                .iter()
                .filter(|block| block.unit_type == "driver_player")
                .count(),
            blocks
                .iter()
                .filter(|block| block.unit_type == "trailer")
                .count(),
        )
    }
}
