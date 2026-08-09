use std::collections::{BTreeMap, HashMap, HashSet};

use crate::features::truck_change::parser::{
    UnitBlock, extract_array_entries, extract_field_value, is_null_ref, parse_unit_blocks,
};
use crate::shared::hex_float::parse_value_auto;

use super::models::{
    GarageInfo, GarageOwnership, GarageParseDiagnostics, GarageSize, GarageSlotInfo,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedGarageList {
    pub headquarters_garage_id: Option<String>,
    pub garages: Vec<GarageInfo>,
    pub diagnostics: GarageParseDiagnostics,
}

pub fn parse_garages_from_sii(content: &str) -> Result<ParsedGarageList, String> {
    let blocks = parse_unit_blocks(content);
    let economy_blocks = blocks
        .iter()
        .filter(|block| block.unit_type == "economy")
        .collect::<Vec<_>>();
    let economy_block = match economy_blocks.as_slice() {
        [] => return Err("garage_block_invalid:economy_missing".to_string()),
        [block] => *block,
        _ => return Err("garage_reference_ambiguous:economy".to_string()),
    };

    let declared_garage_count = parse_required_count(economy_block, "garages")?;
    let garage_entries = extract_array_entries(&economy_block.raw_block, "garages");
    validate_reference_indices(
        economy_block,
        "garages",
        declared_garage_count,
        &garage_entries,
    )?;

    let mut garage_blocks_by_id: HashMap<&str, Vec<&UnitBlock>> = HashMap::new();
    for block in blocks.iter().filter(|block| block.unit_type == "garage") {
        garage_blocks_by_id
            .entry(block.id.as_str())
            .or_default()
            .push(block);
    }

    let referenced_ids = garage_entries
        .iter()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    let referenced_id_set = referenced_ids.iter().cloned().collect::<HashSet<_>>();
    if referenced_id_set.len() != referenced_ids.len() {
        return Err("garage_reference_ambiguous:economy_garages".to_string());
    }

    let mut diagnostics_warnings = Vec::new();
    if declared_garage_count != referenced_ids.len() {
        diagnostics_warnings.push(format!(
            "garage_count_mismatch:declared={declared_garage_count}:referenced={}",
            referenced_ids.len()
        ));
    }

    let headquarters_garage_id = resolve_headquarters_garage_id(
        economy_block,
        &blocks,
        &referenced_id_set,
        &mut diagnostics_warnings,
    )?;
    let mut blocks_by_id: HashMap<&str, Vec<&UnitBlock>> = HashMap::new();
    for block in &blocks {
        blocks_by_id
            .entry(block.id.as_str())
            .or_default()
            .push(block);
    }

    let mut garages = Vec::with_capacity(referenced_ids.len());
    for garage_id in &referenced_ids {
        if city_token_from_garage_id(garage_id).is_none() {
            return Err(format!(
                "garage_block_invalid:{garage_id}:invalid_garage_id"
            ));
        }
        let matching_blocks = garage_blocks_by_id
            .get(garage_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let garage_block = match matching_blocks {
            [] => return Err(format!("garage_not_found:{garage_id}")),
            [block] => *block,
            _ => return Err(format!("garage_reference_ambiguous:{garage_id}")),
        };
        let mut garage = parse_garage_block(
            garage_block,
            headquarters_garage_id.as_deref() == Some(garage_id.as_str()),
        )?;
        validate_assignment_targets(&mut garage, &blocks_by_id);
        garages.push(garage);
    }
    validate_duplicate_assignments(&mut garages);
    validate_headquarters_consistency(&mut garages, headquarters_garage_id.as_deref());
    diagnostics_warnings.extend(
        garages
            .iter()
            .flat_map(|garage| garage.warnings.iter().cloned()),
    );

    let owned_garage_count = garages
        .iter()
        .filter(|garage| garage.ownership == GarageOwnership::Owned)
        .count();
    let not_owned_garage_count = garages
        .iter()
        .filter(|garage| garage.ownership == GarageOwnership::NotOwned)
        .count();
    let unknown_status_count = garages
        .iter()
        .filter(|garage| garage.ownership == GarageOwnership::Unknown)
        .count();
    let unreferenced_garage_count = blocks
        .iter()
        .filter(|block| {
            block.unit_type == "garage" && !referenced_id_set.contains(block.id.as_str())
        })
        .count();
    if unreferenced_garage_count > 0 {
        diagnostics_warnings.push(format!(
            "unreferenced_garage_blocks:{unreferenced_garage_count}"
        ));
    }

    Ok(ParsedGarageList {
        headquarters_garage_id,
        diagnostics: GarageParseDiagnostics {
            declared_garage_count,
            referenced_garage_count: referenced_ids.len(),
            parsed_garage_count: garages.len(),
            owned_garage_count,
            not_owned_garage_count,
            unknown_status_count,
            unreferenced_garage_count,
            warnings: diagnostics_warnings,
        },
        garages,
    })
}

pub fn city_token_from_garage_id(garage_id: &str) -> Option<String> {
    let city_token = garage_id.strip_prefix("garage.")?;
    if city_token.is_empty()
        || !city_token.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return None;
    }
    Some(city_token.to_string())
}

fn parse_garage_block(block: &UnitBlock, is_headquarters: bool) -> Result<GarageInfo, String> {
    let vehicle_slot_count = parse_required_count(block, "vehicles")?;
    let driver_slot_count = parse_required_count(block, "drivers")?;
    let trailer_slot_count = parse_required_count(block, "trailers")?;
    let vehicle_entries = extract_array_entries(&block.raw_block, "vehicles");
    let driver_entries = extract_array_entries(&block.raw_block, "drivers");
    let trailer_entries = extract_array_entries(&block.raw_block, "trailers");
    validate_reference_indices(block, "vehicles", vehicle_slot_count, &vehicle_entries)?;
    validate_reference_indices(block, "drivers", driver_slot_count, &driver_entries)?;
    validate_reference_indices(block, "trailers", trailer_slot_count, &trailer_entries)?;

    let status = extract_field_value(&block.raw_block, "status")
        .ok_or_else(|| format!("garage_block_invalid:{}:status_missing", block.id))?
        .parse::<i32>()
        .map_err(|_| format!("garage_block_invalid:{}:status_invalid", block.id))?;
    let vehicle_map = vehicle_entries.into_iter().collect::<BTreeMap<_, _>>();
    let driver_map = driver_entries.into_iter().collect::<BTreeMap<_, _>>();
    let slot_count = vehicle_slot_count.max(driver_slot_count);
    let slots = (0..slot_count)
        .map(|index| GarageSlotInfo {
            index,
            truck_id: non_null_reference(vehicle_map.get(&index)),
            driver_id: non_null_reference(driver_map.get(&index)),
        })
        .collect::<Vec<_>>();
    let occupied_slots = slots
        .iter()
        .filter(|slot| slot.truck_id.is_some() || slot.driver_id.is_some())
        .count();
    let assigned_truck_count = slots.iter().filter(|slot| slot.truck_id.is_some()).count();
    let assigned_driver_count = slots.iter().filter(|slot| slot.driver_id.is_some()).count();
    let trailer_ids = trailer_entries
        .iter()
        .filter_map(|(_, value)| {
            if is_null_ref(value) {
                None
            } else {
                Some(value.clone())
            }
        })
        .collect::<Vec<_>>();

    let mut warnings = Vec::new();
    let (size, ownership, expected_capacity) = classify_garage(status);
    let capacity_consistent =
        vehicle_slot_count == driver_slot_count && expected_capacity == Some(vehicle_slot_count);
    if !capacity_consistent {
        warnings.push(format!("garage_capacity_mismatch:{}", block.id));
    }
    if ownership == GarageOwnership::Unknown {
        warnings.push(format!("garage_status_unknown:{}:{status}", block.id));
    }

    let productivity = match extract_field_value(&block.raw_block, "productivity") {
        Some(raw_value) => match parse_value_auto(&raw_value) {
            Ok(value) if value.is_finite() => Some(value),
            Ok(_) | Err(_) => {
                warnings.push(format!("garage_productivity_invalid:{}", block.id));
                None
            }
        },
        None => None,
    };
    let profit_log_id =
        extract_field_value(&block.raw_block, "profit_log").filter(|value| !is_null_ref(value));

    Ok(GarageInfo {
        garage_id: block.id.clone(),
        city_token: city_token_from_garage_id(&block.id),
        city_name: None,
        country_code: None,
        status: Some(status),
        size,
        ownership,
        vehicle_slot_count,
        driver_slot_count,
        trailer_slot_count,
        maximum_slot_count: 5,
        occupied_slots,
        available_slots: vehicle_slot_count.saturating_sub(occupied_slots),
        assigned_driver_count,
        assigned_truck_count,
        assigned_trailer_count: trailer_ids.len(),
        slots,
        trailer_ids,
        is_headquarters,
        capacity_consistent,
        profit_log_id,
        productivity,
        warnings,
    })
}

fn parse_required_count(block: &UnitBlock, field: &str) -> Result<usize, String> {
    extract_field_value(&block.raw_block, field)
        .ok_or_else(|| format!("garage_block_invalid:{}:{field}_missing", block.id))?
        .parse::<usize>()
        .map_err(|_| format!("garage_block_invalid:{}:{field}_invalid", block.id))
}

fn validate_reference_indices(
    block: &UnitBlock,
    field: &str,
    declared_count: usize,
    entries: &[(usize, String)],
) -> Result<(), String> {
    if entries.len() != declared_count {
        return Err(format!(
            "garage_block_invalid:{}:{field}_count_mismatch",
            block.id
        ));
    }
    let unique_indices = entries
        .iter()
        .map(|(index, _)| *index)
        .collect::<HashSet<_>>();
    if unique_indices.len() != entries.len()
        || entries.iter().any(|(index, _)| *index >= declared_count)
    {
        return Err(format!(
            "garage_block_invalid:{}:{field}_indices_invalid",
            block.id
        ));
    }
    Ok(())
}

fn non_null_reference(value: Option<&String>) -> Option<String> {
    value.filter(|value| !is_null_ref(value)).cloned()
}

fn classify_garage(status: i32) -> (GarageSize, GarageOwnership, Option<usize>) {
    match status {
        0 => (GarageSize::Unowned, GarageOwnership::NotOwned, Some(0)),
        2 => (GarageSize::Small, GarageOwnership::Owned, Some(3)),
        3 => (GarageSize::Large, GarageOwnership::Owned, Some(5)),
        _ => (GarageSize::Unknown, GarageOwnership::Unknown, None),
    }
}

fn resolve_headquarters_garage_id(
    economy_block: &UnitBlock,
    blocks: &[UnitBlock],
    referenced_garage_ids: &HashSet<String>,
    warnings: &mut Vec<String>,
) -> Result<Option<String>, String> {
    let Some(player_id) = extract_field_value(&economy_block.raw_block, "player") else {
        warnings.push("garage_headquarters_unresolved:player_reference_missing".to_string());
        return Ok(None);
    };
    let player_blocks = blocks
        .iter()
        .filter(|block| block.unit_type == "player" && block.id == player_id)
        .collect::<Vec<_>>();
    let player_block = match player_blocks.as_slice() {
        [] => {
            warnings.push("garage_headquarters_unresolved:player_block_missing".to_string());
            return Ok(None);
        }
        [block] => *block,
        _ => return Err(format!("garage_reference_ambiguous:{player_id}")),
    };
    let hq_city_field_count = player_block
        .raw_block
        .lines()
        .filter(|line| line.trim_start().starts_with("hq_city:"))
        .count();
    let hq_city = match hq_city_field_count {
        0 => {
            warnings.push("garage_headquarters_unresolved:hq_city_missing".to_string());
            return Ok(None);
        }
        1 => extract_field_value(&player_block.raw_block, "hq_city")
            .ok_or_else(|| "garage_block_invalid:hq_city_invalid".to_string())?,
        _ => return Err("garage_reference_ambiguous:hq_city".to_string()),
    };
    let garage_id = format!("garage.{hq_city}");
    if !referenced_garage_ids.contains(&garage_id) {
        warnings.push(format!(
            "garage_headquarters_unresolved:garage_not_referenced:{garage_id}"
        ));
        return Ok(None);
    }
    Ok(Some(garage_id))
}

fn validate_assignment_targets(
    garage: &mut GarageInfo,
    blocks_by_id: &HashMap<&str, Vec<&UnitBlock>>,
) {
    match garage.profit_log_id.as_deref() {
        Some(profit_log_id) if reference_is_ambiguous(blocks_by_id, profit_log_id) => {
            garage.warnings.push(format!(
                "garage_profit_log_reference_ambiguous:{}",
                garage.garage_id
            ));
        }
        Some(profit_log_id)
            if !reference_has_type(blocks_by_id, profit_log_id, &["profit_log"]) =>
        {
            garage.warnings.push(format!(
                "garage_profit_log_reference_unresolved:{}",
                garage.garage_id
            ));
        }
        None => garage.warnings.push(format!(
            "garage_profit_log_reference_unresolved:{}",
            garage.garage_id
        )),
        Some(_) => {}
    }

    for slot in &garage.slots {
        if let Some(truck_id) = slot.truck_id.as_deref() {
            if reference_is_ambiguous(blocks_by_id, truck_id) {
                garage.warnings.push(format!(
                    "garage_truck_reference_ambiguous:{}:{}",
                    garage.garage_id, slot.index
                ));
            } else if !reference_has_type(blocks_by_id, truck_id, &["vehicle"]) {
                garage.warnings.push(format!(
                    "garage_truck_reference_unresolved:{}:{}",
                    garage.garage_id, slot.index
                ));
            }
        }
        if let Some(driver_id) = slot.driver_id.as_deref() {
            if reference_is_ambiguous(blocks_by_id, driver_id) {
                garage.warnings.push(format!(
                    "garage_driver_reference_ambiguous:{}:{}",
                    garage.garage_id, slot.index
                ));
            } else if !reference_has_type(blocks_by_id, driver_id, &["driver_ai", "driver_player"])
            {
                garage.warnings.push(format!(
                    "garage_driver_reference_unresolved:{}:{}",
                    garage.garage_id, slot.index
                ));
            }
        }
        if slot.driver_id.is_some() && slot.truck_id.is_none() {
            garage.warnings.push(format!(
                "garage_slot_assignment_inconsistent:{}:{}:driver_without_truck",
                garage.garage_id, slot.index
            ));
        }
    }
    for (index, trailer_id) in garage.trailer_ids.iter().enumerate() {
        if reference_is_ambiguous(blocks_by_id, trailer_id) {
            garage.warnings.push(format!(
                "garage_trailer_reference_ambiguous:{}:{index}",
                garage.garage_id
            ));
        } else if !reference_has_type(blocks_by_id, trailer_id, &["trailer"]) {
            garage.warnings.push(format!(
                "garage_trailer_reference_unresolved:{}:{index}",
                garage.garage_id
            ));
        }
    }
}

fn reference_is_ambiguous(blocks_by_id: &HashMap<&str, Vec<&UnitBlock>>, reference: &str) -> bool {
    blocks_by_id
        .get(reference)
        .is_some_and(|blocks| blocks.len() > 1)
}

fn reference_has_type(
    blocks_by_id: &HashMap<&str, Vec<&UnitBlock>>,
    reference: &str,
    expected_types: &[&str],
) -> bool {
    let Some(blocks) = blocks_by_id.get(reference) else {
        return false;
    };
    blocks.len() == 1 && expected_types.contains(&blocks[0].unit_type.as_str())
}

fn validate_duplicate_assignments(garages: &mut [GarageInfo]) {
    let mut truck_locations: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut driver_locations: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut trailer_locations: HashMap<String, Vec<(usize, usize)>> = HashMap::new();

    for (garage_index, garage) in garages.iter().enumerate() {
        for slot in &garage.slots {
            if let Some(truck_id) = slot.truck_id.as_ref() {
                truck_locations
                    .entry(truck_id.clone())
                    .or_default()
                    .push((garage_index, slot.index));
            }
            if let Some(driver_id) = slot.driver_id.as_ref() {
                driver_locations
                    .entry(driver_id.clone())
                    .or_default()
                    .push((garage_index, slot.index));
            }
        }
        for (trailer_index, trailer_id) in garage.trailer_ids.iter().enumerate() {
            trailer_locations
                .entry(trailer_id.clone())
                .or_default()
                .push((garage_index, trailer_index));
        }
    }

    apply_duplicate_assignment_warnings(garages, &truck_locations, "truck");
    apply_duplicate_assignment_warnings(garages, &driver_locations, "driver");
    apply_duplicate_assignment_warnings(garages, &trailer_locations, "trailer");
}

fn validate_headquarters_consistency(
    garages: &mut [GarageInfo],
    headquarters_garage_id: Option<&str>,
) {
    let Some(headquarters_garage_id) = headquarters_garage_id else {
        return;
    };
    let Some(headquarters) = garages
        .iter_mut()
        .find(|garage| garage.garage_id == headquarters_garage_id)
    else {
        return;
    };
    if headquarters.ownership != GarageOwnership::Owned {
        headquarters.warnings.push(format!(
            "garage_headquarters_not_owned:{}",
            headquarters.garage_id
        ));
    }
}

fn apply_duplicate_assignment_warnings(
    garages: &mut [GarageInfo],
    locations_by_id: &HashMap<String, Vec<(usize, usize)>>,
    reference_type: &str,
) {
    for locations in locations_by_id.values().filter(|items| items.len() > 1) {
        for (garage_index, slot_index) in locations {
            let garage = &mut garages[*garage_index];
            let warning = format!(
                "garage_{reference_type}_reference_duplicate:{}:{slot_index}",
                garage.garage_id
            );
            if !garage.warnings.contains(&warning) {
                garage.warnings.push(warning);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{city_token_from_garage_id, parse_garages_from_sii};
    use crate::features::garages::models::{GarageOwnership, GarageSize};

    const SAMPLE: &str = include_str!("../../../test-fixtures/garages/garage_samples.sii");
    const REAL_SAMPLE: &str = include_str!("../../../test-fixtures/decrypt/plain_game.sii");

    fn single_garage_sii() -> &'static str {
        r#"SiiNunit
{
economy : _economy {
 player: _player
 garages: 1
 garages[0]: garage.berlin
}
player : _player {
 hq_city: berlin
}
garage : garage.berlin {
 vehicles: 0
 drivers: 0
 trailers: 0
 status: 0
 profit_log: null
 productivity: 0
}
}"#
    }

    #[test]
    fn parses_single_garage() {
        let parsed = parse_garages_from_sii(single_garage_sii()).unwrap();
        assert_eq!(parsed.garages.len(), 1);
        assert_eq!(parsed.garages[0].garage_id, "garage.berlin");
        assert!(parsed.garages[0].is_headquarters);
    }

    #[test]
    fn preserves_economy_order_for_multiple_garages() {
        let parsed = parse_garages_from_sii(SAMPLE).unwrap();
        let ids = parsed
            .garages
            .iter()
            .map(|garage| garage.garage_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "garage.berlin",
                "garage.paris",
                "garage.los_angeles",
                "garage.unknown_city"
            ]
        );
    }

    #[test]
    fn resolves_truck_driver_and_trailer_references() {
        let parsed = parse_garages_from_sii(SAMPLE).unwrap();
        let garage = &parsed.garages[0];
        assert_eq!(garage.assigned_truck_count, 2);
        assert_eq!(garage.assigned_driver_count, 1);
        assert_eq!(garage.assigned_trailer_count, 1);
        assert_eq!(garage.trailer_slot_count, 1);
        assert_eq!(garage.occupied_slots, 2);
        assert_eq!(garage.available_slots, 3);
        assert_eq!(garage.slots[0].truck_id.as_deref(), Some("truck.one"));
        assert_eq!(garage.slots[0].driver_id.as_deref(), Some("driver.one"));
        assert_eq!(garage.trailer_ids, vec!["trailer.one"]);
        assert!(
            !garage
                .warnings
                .iter()
                .any(|warning| warning.contains("reference_unresolved"))
        );
    }

    #[test]
    fn parses_garage_without_driver() {
        let parsed = parse_garages_from_sii(SAMPLE).unwrap();
        let garage = &parsed.garages[1];
        assert_eq!(garage.assigned_driver_count, 0);
        assert_eq!(garage.assigned_truck_count, 0);
        assert_eq!(garage.available_slots, 3);
    }

    #[test]
    fn ignores_unknown_garage_fields() {
        let parsed = parse_garages_from_sii(SAMPLE).unwrap();
        assert_eq!(parsed.garages[0].garage_id, "garage.berlin");
    }

    #[test]
    fn rejects_invalid_garage_id() {
        let invalid = single_garage_sii().replace("garage.berlin", "invalid.berlin");
        let error = parse_garages_from_sii(&invalid).unwrap_err();
        assert!(error.contains("invalid_garage_id"));
    }

    #[test]
    fn reports_missing_garage_reference() {
        let invalid = SAMPLE.replace("garage : garage.paris", "garage : garage.lyon");
        let error = parse_garages_from_sii(&invalid).unwrap_err();
        assert_eq!(error, "garage_not_found:garage.paris");
    }

    #[test]
    fn reports_ambiguous_garage_reference() {
        let duplicate = format!(
            r#"{SAMPLE}
garage : garage.paris {{
 vehicles: 0
 drivers: 0
 trailers: 0
 status: 0
}}
"#
        );
        let error = parse_garages_from_sii(&duplicate).unwrap_err();
        assert_eq!(error, "garage_reference_ambiguous:garage.paris");
    }

    #[test]
    fn does_not_guess_unknown_status_or_capacity() {
        let parsed = parse_garages_from_sii(SAMPLE).unwrap();
        let garage = &parsed.garages[3];
        assert_eq!(garage.size, GarageSize::Unknown);
        assert_eq!(garage.ownership, GarageOwnership::Unknown);
        assert!(!garage.capacity_consistent);
    }

    #[test]
    fn parses_hex_productivity() {
        let parsed = parse_garages_from_sii(SAMPLE).unwrap();
        assert_eq!(parsed.garages[0].productivity, Some(0.5));
        assert_eq!(parsed.garages[3].productivity, None);
    }

    #[test]
    fn validates_city_tokens() {
        assert_eq!(
            city_token_from_garage_id("garage.los_angeles").as_deref(),
            Some("los_angeles")
        );
        assert_eq!(city_token_from_garage_id("garage.Bad City"), None);
        assert_eq!(city_token_from_garage_id("company.berlin"), None);
    }

    #[test]
    fn parses_existing_decrypted_ets2_fixture() {
        let parsed = parse_garages_from_sii(REAL_SAMPLE).unwrap();
        assert_eq!(parsed.garages.len(), 222);
        assert_eq!(
            parsed.headquarters_garage_id.as_deref(),
            Some("garage.lille")
        );
        assert!(
            parsed
                .garages
                .iter()
                .all(|garage| matches!(garage.status, Some(0) | Some(2) | Some(3)))
        );
        assert!(parsed.garages.iter().all(|garage| {
            garage.profit_log_id.is_some()
                && !garage.warnings.iter().any(|warning| {
                    warning.starts_with("garage_profit_log_reference_unresolved")
                        || warning.starts_with("garage_profit_log_reference_ambiguous")
                })
        }));

        let partially_occupied = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.lille")
            .unwrap();
        assert_eq!(partially_occupied.status, Some(3));
        assert_eq!(partially_occupied.vehicle_slot_count, 5);
        assert_eq!(partially_occupied.driver_slot_count, 5);
        assert!(partially_occupied.assigned_truck_count > 0);
        assert!(partially_occupied.assigned_driver_count > 0);
        assert!(
            partially_occupied
                .slots
                .iter()
                .any(|slot| slot.driver_id.is_none())
        );
    }

    #[test]
    fn reports_missing_profit_log_block() {
        let invalid = SAMPLE.replace(
            "profit_log : profit.los_angeles",
            "profit_log : profit.detached",
        );
        let parsed = parse_garages_from_sii(&invalid).unwrap();
        let garage = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.los_angeles")
            .unwrap();
        assert!(garage.warnings.iter().any(|warning| {
            warning == "garage_profit_log_reference_unresolved:garage.los_angeles"
        }));
    }

    #[test]
    fn reports_ambiguous_assignment_unit_reference() {
        let duplicate = format!("{SAMPLE}\nvehicle : truck.one {{\n}}\n");
        let parsed = parse_garages_from_sii(&duplicate).unwrap();
        let garage = &parsed.garages[0];
        assert!(
            garage
                .warnings
                .iter()
                .any(|warning| warning.starts_with("garage_truck_reference_ambiguous"))
        );
    }

    #[test]
    fn reports_duplicate_assignments() {
        let duplicate = SAMPLE.replace("vehicles[2]: null", "vehicles[2]: truck.one");
        let parsed = parse_garages_from_sii(&duplicate).unwrap();
        let garage = &parsed.garages[0];
        assert!(
            garage
                .warnings
                .iter()
                .any(|warning| warning.starts_with("garage_truck_reference_duplicate"))
        );
    }

    #[test]
    fn reports_driver_assignment_without_truck() {
        let inconsistent = SAMPLE.replace("drivers[2]: null", "drivers[2]: driver.one");
        let parsed = parse_garages_from_sii(&inconsistent).unwrap();
        let garage = &parsed.garages[0];
        assert!(
            garage
                .warnings
                .iter()
                .any(|warning| { warning.starts_with("garage_slot_assignment_inconsistent") })
        );
    }

    #[test]
    fn rejects_duplicate_headquarters_fields() {
        let invalid = SAMPLE.replace(" hq_city: berlin", " hq_city: berlin\n hq_city: paris");
        let error = parse_garages_from_sii(&invalid).unwrap_err();
        assert_eq!(error, "garage_reference_ambiguous:hq_city");
    }

    #[test]
    fn rejects_corrupted_garage_structure() {
        let invalid = SAMPLE.replacen(" drivers: 5", "", 1);
        let error = parse_garages_from_sii(&invalid).unwrap_err();
        assert_eq!(error, "garage_block_invalid:garage.berlin:drivers_missing");
    }
}
