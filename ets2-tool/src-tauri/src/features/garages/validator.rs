use crate::features::truck_change::parser::{
    UnitBlock, extract_array_entries, extract_field_value, parse_unit_blocks,
};

use super::models::{GarageInfo, GarageOperation, GarageOwnership, GarageSize};
use super::parser::parse_garages_from_sii;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GarageVerificationSpec {
    pub operation: GarageOperation,
    pub target_size: Option<GarageSize>,
    pub set_as_headquarters: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedGarageMutation {
    pub previous_state: GarageInfo,
    pub updated_state: GarageInfo,
}

pub fn verify_garage_mutation(
    before_content: &str,
    after_content: &str,
    garage_id: &str,
    spec: &GarageVerificationSpec,
) -> Result<VerifiedGarageMutation, String> {
    let before = parse_garages_from_sii(before_content)
        .map_err(|error| format!("save_verification_failed:{error}"))?;
    let after = parse_garages_from_sii(after_content)
        .map_err(|error| format!("save_verification_failed:{error}"))?;
    let previous_state = find_garage(&before.garages, garage_id)?;
    let updated_state = find_garage(&after.garages, garage_id)?;

    let before_ids = before
        .garages
        .iter()
        .map(|garage| garage.garage_id.as_str())
        .collect::<Vec<_>>();
    let after_ids = after
        .garages
        .iter()
        .map(|garage| garage.garage_id.as_str())
        .collect::<Vec<_>>();
    if before_ids != after_ids {
        return Err("save_verification_failed:garage_inventory_changed".to_string());
    }

    verify_untouched_garages(
        &before.garages,
        &after.garages,
        garage_id,
        spec.set_as_headquarters,
    )?;
    verify_unit_blocks_unchanged(before_content, after_content, garage_id, spec)?;
    verify_target_garage_metadata_unchanged(before_content, after_content, garage_id)?;
    verify_target_references(&previous_state, &updated_state, spec)?;
    verify_expected_state(&previous_state, &updated_state, spec)?;
    verify_raw_expected_state(after_content, garage_id, &updated_state, spec)?;
    verify_headquarters_transition(&before.garages, &after.garages, garage_id, spec)?;

    Ok(VerifiedGarageMutation {
        previous_state,
        updated_state,
    })
}

fn verify_headquarters_transition(
    before: &[GarageInfo],
    after: &[GarageInfo],
    target_garage_id: &str,
    spec: &GarageVerificationSpec,
) -> Result<(), String> {
    let before_headquarters = before
        .iter()
        .filter(|garage| garage.is_headquarters)
        .collect::<Vec<_>>();
    let after_headquarters = after
        .iter()
        .filter(|garage| garage.is_headquarters)
        .collect::<Vec<_>>();

    if spec.set_as_headquarters {
        if before_headquarters.len() != 1 {
            return Err("save_verification_failed:hq_count_before".to_string());
        }
        if after_headquarters.len() != 1 {
            return Err("save_verification_failed:hq_count_after".to_string());
        }
        let new_headquarters = after_headquarters[0];
        if new_headquarters.garage_id != target_garage_id {
            return Err("save_verification_failed:hq_not_updated".to_string());
        }
        if new_headquarters.ownership != GarageOwnership::Owned {
            return Err("save_verification_failed:hq_not_owned".to_string());
        }
        let previous_headquarters = before_headquarters[0];
        if previous_headquarters.ownership != GarageOwnership::Owned {
            return Err("save_verification_failed:previous_hq_not_owned".to_string());
        }
        let previous_after = after
            .iter()
            .find(|garage| garage.garage_id == previous_headquarters.garage_id)
            .ok_or_else(|| "save_verification_failed:previous_hq_missing".to_string())?;
        if previous_headquarters.garage_id != target_garage_id && previous_after.is_headquarters {
            return Err("save_verification_failed:previous_hq_retained".to_string());
        }
    } else {
        let before_id = before_headquarters
            .first()
            .map(|garage| garage.garage_id.as_str());
        let after_id = after_headquarters
            .first()
            .map(|garage| garage.garage_id.as_str());
        if before_headquarters.len() != after_headquarters.len() || before_id != after_id {
            return Err("save_verification_failed:hq_changed_unexpectedly".to_string());
        }
    }
    Ok(())
}

fn find_garage(garages: &[GarageInfo], garage_id: &str) -> Result<GarageInfo, String> {
    garages
        .iter()
        .find(|garage| garage.garage_id == garage_id)
        .cloned()
        .ok_or_else(|| format!("garage_not_found:{garage_id}"))
}

fn verify_untouched_garages(
    before: &[GarageInfo],
    after: &[GarageInfo],
    target_garage_id: &str,
    allow_hq_change: bool,
) -> Result<(), String> {
    for previous in before {
        if previous.garage_id == target_garage_id {
            continue;
        }
        let Some(updated) = after
            .iter()
            .find(|garage| garage.garage_id == previous.garage_id)
        else {
            return Err("save_verification_failed:garage_inventory_changed".to_string());
        };
        let mut normalized_previous = previous.clone();
        let mut normalized_updated = updated.clone();
        if allow_hq_change {
            normalized_previous.is_headquarters = false;
            normalized_updated.is_headquarters = false;
        }
        if normalized_previous != normalized_updated {
            return Err(format!(
                "save_verification_failed:unrelated_garage_changed:{}",
                previous.garage_id
            ));
        }
    }
    Ok(())
}

fn verify_unit_blocks_unchanged(
    before_content: &str,
    after_content: &str,
    target_garage_id: &str,
    spec: &GarageVerificationSpec,
) -> Result<(), String> {
    let before_blocks = parse_unit_blocks(before_content);
    let after_blocks = parse_unit_blocks(after_content);
    if before_blocks.len() != after_blocks.len() {
        return Err("save_verification_failed:unit_inventory_changed".to_string());
    }
    let player_id = if spec.set_as_headquarters {
        Some(resolve_player_id(before_content)?)
    } else {
        None
    };
    let garage_block_may_change =
        !matches!(spec.operation, GarageOperation::Update) || spec.target_size.is_some();

    for (previous, updated) in before_blocks.iter().zip(after_blocks.iter()) {
        if previous.unit_type != updated.unit_type || previous.id != updated.id {
            return Err("save_verification_failed:unit_order_changed".to_string());
        }
        let allowed_garage_change = garage_block_may_change && previous.id == target_garage_id;
        let allowed_player_change = player_id.as_deref() == Some(previous.id.as_str());
        if !allowed_garage_change
            && !allowed_player_change
            && previous.raw_block != updated.raw_block
        {
            return Err(format!(
                "save_verification_failed:unrelated_unit_changed:{}",
                previous.id
            ));
        }
    }
    Ok(())
}

fn resolve_player_id(content: &str) -> Result<String, String> {
    let economy_blocks = parse_unit_blocks(content)
        .into_iter()
        .filter(|block| block.unit_type == "economy")
        .collect::<Vec<_>>();
    let economy = match economy_blocks.as_slice() {
        [block] => block,
        [] => return Err("save_verification_failed:economy_missing".to_string()),
        _ => return Err("save_verification_failed:economy_ambiguous".to_string()),
    };
    extract_field_value(&economy.raw_block, "player")
        .ok_or_else(|| "save_verification_failed:player_reference_missing".to_string())
}

fn verify_target_garage_metadata_unchanged(
    before_content: &str,
    after_content: &str,
    garage_id: &str,
) -> Result<(), String> {
    let before = unique_garage_block(before_content, garage_id)?;
    let after = unique_garage_block(after_content, garage_id)?;
    if immutable_garage_lines(&before) != immutable_garage_lines(&after) {
        return Err("save_verification_failed:garage_metadata_changed".to_string());
    }
    Ok(())
}

fn immutable_garage_lines(block: &UnitBlock) -> Vec<&str> {
    block
        .raw_block
        .lines()
        .filter(|line| !is_mutable_garage_line(line))
        .collect()
}

fn is_mutable_garage_line(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("status:")
        || line.starts_with("vehicles:")
        || line.starts_with("vehicles[")
        || line.starts_with("drivers:")
        || line.starts_with("drivers[")
}

fn unique_garage_block(content: &str, garage_id: &str) -> Result<UnitBlock, String> {
    let matching = parse_unit_blocks(content)
        .into_iter()
        .filter(|block| block.unit_type == "garage" && block.id == garage_id)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [block] => Ok(block.clone()),
        [] => Err(format!("garage_not_found:{garage_id}")),
        _ => Err(format!("garage_reference_ambiguous:{garage_id}")),
    }
}

fn verify_target_references(
    previous: &GarageInfo,
    updated: &GarageInfo,
    spec: &GarageVerificationSpec,
) -> Result<(), String> {
    if previous.trailer_ids != updated.trailer_ids
        || previous.trailer_slot_count != updated.trailer_slot_count
        || previous.assigned_trailer_count != updated.assigned_trailer_count
    {
        return Err("save_verification_failed:trailer_references_changed".to_string());
    }
    if previous.profit_log_id != updated.profit_log_id
        || previous.productivity != updated.productivity
    {
        return Err("save_verification_failed:garage_metadata_changed".to_string());
    }
    for garage in [previous, updated] {
        if garage.profit_log_id.is_none()
            || garage.warnings.iter().any(|warning| {
                warning.starts_with("garage_profit_log_reference_unresolved")
                    || warning.starts_with("garage_profit_log_reference_ambiguous")
            })
        {
            return Err("save_verification_failed:profit_log_reference_invalid".to_string());
        }
    }
    let allow_empty_slot_removal = matches!(spec.operation, GarageOperation::Update)
        && spec.target_size == Some(GarageSize::Small);
    for previous_slot in &previous.slots {
        let Some(updated_slot) = updated
            .slots
            .iter()
            .find(|slot| slot.index == previous_slot.index)
        else {
            if allow_empty_slot_removal
                && previous_slot.truck_id.is_none()
                && previous_slot.driver_id.is_none()
            {
                continue;
            }
            return Err("save_verification_failed:garage_slot_removed".to_string());
        };
        if previous_slot.truck_id != updated_slot.truck_id {
            return Err("save_verification_failed:truck_references_changed".to_string());
        }
        if previous_slot.driver_id != updated_slot.driver_id {
            return Err("save_verification_failed:driver_references_changed".to_string());
        }
    }
    if updated
        .slots
        .iter()
        .skip(previous.slots.len())
        .any(|slot| slot.truck_id.is_some() || slot.driver_id.is_some())
    {
        return Err("save_verification_failed:new_slots_not_empty".to_string());
    }
    Ok(())
}

fn verify_raw_expected_state(
    content: &str,
    garage_id: &str,
    updated: &GarageInfo,
    spec: &GarageVerificationSpec,
) -> Result<(), String> {
    let block = unique_garage_block(content, garage_id)?;
    let status =
        extract_field_value(&block.raw_block, "status").and_then(|value| value.parse::<i32>().ok());
    if status != updated.status {
        return Err("garage_size_change_not_verified:status".to_string());
    }
    for (field, expected_count) in [
        ("vehicles", updated.vehicle_slot_count),
        ("drivers", updated.driver_slot_count),
    ] {
        let count = extract_field_value(&block.raw_block, field)
            .and_then(|value| value.parse::<usize>().ok());
        let entries = extract_array_entries(&block.raw_block, field);
        if count != Some(expected_count)
            || entries.len() != expected_count
            || (0..expected_count)
                .any(|index| !entries.iter().any(|(entry_index, _)| *entry_index == index))
        {
            return Err(format!("garage_size_change_not_verified:{field}"));
        }
        if matches!(spec.operation, GarageOperation::Purchase)
            && entries.iter().any(|(_, value)| value.trim() != "null")
        {
            return Err(format!(
                "garage_size_change_not_verified:{field}_slots_not_null"
            ));
        }
    }
    Ok(())
}

fn verify_expected_state(
    previous: &GarageInfo,
    updated: &GarageInfo,
    spec: &GarageVerificationSpec,
) -> Result<(), String> {
    match spec.operation {
        GarageOperation::Purchase => {
            require_state(
                previous,
                GarageOwnership::NotOwned,
                GarageSize::Unowned,
                0,
                0,
            )?;
            require_state(updated, GarageOwnership::Owned, GarageSize::Large, 3, 5)?;
            if previous.trailer_slot_count != 0 || updated.trailer_slot_count != 0 {
                return Err("save_verification_failed:garage_trailers_changed".to_string());
            }
        }
        GarageOperation::Upgrade => {
            require_state(previous, GarageOwnership::Owned, GarageSize::Small, 2, 3)?;
            require_state(updated, GarageOwnership::Owned, GarageSize::Large, 3, 5)?;
        }
        GarageOperation::Update => {
            if previous.ownership != GarageOwnership::Owned
                || updated.ownership != GarageOwnership::Owned
            {
                return Err("garage_not_owned".to_string());
            }
            match spec.target_size {
                Some(GarageSize::Large) => {
                    require_state(updated, GarageOwnership::Owned, GarageSize::Large, 3, 5)?;
                }
                Some(GarageSize::Small) => {
                    require_state(updated, GarageOwnership::Owned, GarageSize::Small, 2, 3)?;
                }
                Some(_) => return Err("garage_size_invalid".to_string()),
                None => {
                    if previous.size != updated.size
                        || previous.status != updated.status
                        || previous.vehicle_slot_count != updated.vehicle_slot_count
                        || previous.driver_slot_count != updated.driver_slot_count
                    {
                        return Err(
                            "save_verification_failed:garage_size_changed_unexpectedly".to_string()
                        );
                    }
                }
            }
        }
    }
    if updated.assigned_driver_count != previous.assigned_driver_count
        || updated.assigned_truck_count != previous.assigned_truck_count
    {
        return Err("save_verification_failed:assignment_counts_changed".to_string());
    }
    Ok(())
}

fn require_state(
    garage: &GarageInfo,
    ownership: GarageOwnership,
    size: GarageSize,
    status: i32,
    capacity: usize,
) -> Result<(), String> {
    if garage.ownership != ownership
        || garage.size != size
        || garage.status != Some(status)
        || garage.vehicle_slot_count != capacity
        || garage.driver_slot_count != capacity
        || !garage.capacity_consistent
    {
        return Err("save_verification_failed:garage_state_mismatch".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{GarageVerificationSpec, verify_garage_mutation};
    use crate::features::garages::models::{GarageOperation, GarageSize};
    use crate::features::garages::writer::apply_garage_changes;

    const SAMPLE: &str = include_str!("../../../test-fixtures/garages/garage_samples.sii");

    #[test]
    fn verifies_purchase() {
        let plan = apply_garage_changes(SAMPLE, "garage.los_angeles", Some((3, 5)), false).unwrap();
        let verified = verify_garage_mutation(
            SAMPLE,
            &plan.content,
            "garage.los_angeles",
            &GarageVerificationSpec {
                operation: GarageOperation::Purchase,
                target_size: Some(GarageSize::Large),
                set_as_headquarters: false,
            },
        )
        .unwrap();
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
    }

    #[test]
    fn verifies_upgrade_and_preserved_references() {
        let plan = apply_garage_changes(SAMPLE, "garage.paris", Some((3, 5)), false).unwrap();
        let verified = verify_garage_mutation(
            SAMPLE,
            &plan.content,
            "garage.paris",
            &GarageVerificationSpec {
                operation: GarageOperation::Upgrade,
                target_size: Some(GarageSize::Large),
                set_as_headquarters: false,
            },
        )
        .unwrap();
        assert_eq!(verified.updated_state.size, GarageSize::Large);
    }

    #[test]
    fn verifies_headquarters_change() {
        let plan = apply_garage_changes(SAMPLE, "garage.paris", None, true).unwrap();
        let verified = verify_garage_mutation(
            SAMPLE,
            &plan.content,
            "garage.paris",
            &GarageVerificationSpec {
                operation: GarageOperation::Update,
                target_size: None,
                set_as_headquarters: true,
            },
        )
        .unwrap();
        assert!(verified.updated_state.is_headquarters);
        let parsed =
            crate::features::garages::parser::parse_garages_from_sii(&plan.content).unwrap();
        assert_eq!(
            parsed
                .garages
                .iter()
                .filter(|garage| garage.is_headquarters)
                .count(),
            1
        );
        let previous_headquarters = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();
        assert!(!previous_headquarters.is_headquarters);
        assert_eq!(
            previous_headquarters.ownership,
            crate::features::garages::models::GarageOwnership::Owned
        );
    }

    #[test]
    fn rejects_duplicate_headquarters_fields_after_write() {
        let plan = apply_garage_changes(SAMPLE, "garage.paris", None, true).unwrap();
        let invalid = plan
            .content
            .replace(" hq_city: paris", " hq_city: paris\n hq_city: berlin");
        let error = verify_garage_mutation(
            SAMPLE,
            &invalid,
            "garage.paris",
            &GarageVerificationSpec {
                operation: GarageOperation::Update,
                target_size: None,
                set_as_headquarters: true,
            },
        )
        .unwrap_err();
        assert!(error.contains("garage_reference_ambiguous:hq_city"));
    }

    #[test]
    fn verifies_downgrade_and_preserved_references() {
        let plan = apply_garage_changes(SAMPLE, "garage.berlin", Some((2, 3)), false).unwrap();
        let verified = verify_garage_mutation(
            SAMPLE,
            &plan.content,
            "garage.berlin",
            &GarageVerificationSpec {
                operation: GarageOperation::Update,
                target_size: Some(GarageSize::Small),
                set_as_headquarters: false,
            },
        )
        .unwrap();

        assert_eq!(verified.updated_state.size, GarageSize::Small);
        assert_eq!(verified.updated_state.assigned_truck_count, 2);
        assert_eq!(verified.updated_state.assigned_driver_count, 1);
        assert_eq!(verified.updated_state.trailer_ids, vec!["trailer.one"]);
    }

    #[test]
    fn rejects_target_garage_metadata_change() {
        let plan = apply_garage_changes(SAMPLE, "garage.berlin", Some((2, 3)), false).unwrap();
        let invalid = plan.content.replace(
            "future_garage_field: preserved_by_reader",
            "future_garage_field: changed",
        );
        let error = verify_garage_mutation(
            SAMPLE,
            &invalid,
            "garage.berlin",
            &GarageVerificationSpec {
                operation: GarageOperation::Update,
                target_size: Some(GarageSize::Small),
                set_as_headquarters: false,
            },
        )
        .unwrap_err();

        assert_eq!(error, "save_verification_failed:garage_metadata_changed");
    }

    #[test]
    fn rejects_unrelated_block_change() {
        let plan = apply_garage_changes(SAMPLE, "garage.paris", Some((3, 5)), false).unwrap();
        let invalid = plan.content.replacen(
            "future_garage_field: preserved_by_reader",
            "future_garage_field: changed",
            1,
        );
        let error = verify_garage_mutation(
            SAMPLE,
            &invalid,
            "garage.paris",
            &GarageVerificationSpec {
                operation: GarageOperation::Upgrade,
                target_size: Some(GarageSize::Large),
                set_as_headquarters: false,
            },
        )
        .unwrap_err();
        assert!(error.contains("unrelated_unit_changed"));
    }
}
