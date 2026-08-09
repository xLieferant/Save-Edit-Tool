use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::features::truck_change::parser::{
    UnitBlock, extract_array_entries, extract_array_values, extract_field_value, is_null_ref,
    normalize_sii_unit_id, parse_unit_blocks,
};
use crate::features::vehicles::license_plate::license_plate_display_text;
use crate::models::trailers::TrailerData;
use crate::shared::sii_parser::{
    get_player_id, parse_trailer_defs_from_sii, parse_trailers_from_sii,
};

use super::models::{
    CurrentTrailerPointer, CurrentTrailerPointerDiagnostics, CurrentTrailerPointerKind,
    OwnedTrailerDiagnostics, PlayerTrailerSlotAssignment, TrailerInventoryItem,
};

#[derive(Debug, Clone)]
pub struct ParsedTrailerSave {
    pub active_trailer_id: Option<String>,
    pub current_trailer_pointer: Option<CurrentTrailerPointer>,
    pub current_trailer_diagnostics: CurrentTrailerPointerDiagnostics,
    pub player_id: Option<String>,
    pub trailer_order: Vec<String>,
    pub trailers: Vec<TrailerInventoryItem>,
    pub player_vehicle_slots: Vec<PlayerTrailerSlotAssignment>,
    pub player_vehicle_assignments: HashMap<String, PlayerTrailerSlotAssignment>,
    pub diagnostics: OwnedTrailerDiagnostics,
    pub unit_ids: HashSet<String>,
    pub unit_blocks: HashMap<String, UnitBlock>,
    pub trailer_blocks: HashMap<String, UnitBlock>,
}

struct CurrentTrailerResolution {
    pointer: Option<CurrentTrailerPointer>,
    diagnostics: CurrentTrailerPointerDiagnostics,
}

struct OwnedTrailerCollection {
    owned_ids: Vec<String>,
    diagnostics: OwnedTrailerDiagnostics,
}

pub fn parse_trailer_save(content: &str) -> ParsedTrailerSave {
    let unit_blocks = parse_unit_blocks(content);
    let unit_ids = unit_blocks
        .iter()
        .map(|block| block.id.clone())
        .collect::<HashSet<_>>();
    let blocks_by_id = unit_blocks
        .iter()
        .map(|block| (block.id.clone(), block.clone()))
        .collect::<HashMap<_, _>>();
    let trailer_blocks = unit_blocks
        .iter()
        .filter(|block| block.unit_type == "trailer")
        .map(|block| (block.id.clone(), block.clone()))
        .collect::<HashMap<_, _>>();
    let player_id = get_player_id(content);
    let player_block = player_id
        .as_ref()
        .and_then(|id| find_unit_block_by_id(&blocks_by_id, id, Some("player")))
        .cloned();
    let trailer_order = player_block
        .as_ref()
        .map(|block| extract_array_values(&block.raw_block, "trailers"))
        .unwrap_or_default()
        .into_iter()
        .filter(|value| !is_null_ref(value))
        .collect::<Vec<_>>();
    let player_vehicle_slots = parse_player_vehicle_slots(player_block.as_ref(), &unit_blocks);
    let current_resolution = resolve_current_trailer_pointer_from_parts(
        player_block.as_ref(),
        &blocks_by_id,
        &trailer_blocks,
        &trailer_order,
        &player_vehicle_slots.assignments,
    );
    let active_trailer_id = current_resolution
        .pointer
        .as_ref()
        .map(|pointer| pointer.trailer_id.clone());
    let owned_collection = collect_owned_player_trailer_ids(
        &trailer_order,
        active_trailer_id.as_deref(),
        &trailer_blocks,
    );
    let mut diagnostics = owned_collection.diagnostics;
    diagnostics.current_trailer_pointer_kind = current_resolution
        .pointer
        .as_ref()
        .map(|pointer| pointer.kind.clone());
    diagnostics.current_trailer_id = active_trailer_id.clone();
    diagnostics.assigned_vehicles_unit_id = current_resolution
        .pointer
        .as_ref()
        .and_then(|pointer| pointer.referenced_player_vehicle_unit_id.clone());
    diagnostics.current_trailer_pointer = current_resolution.diagnostics.clone();
    diagnostics.current_trailer_source = current_resolution
        .pointer
        .as_ref()
        .map(|pointer| pointer.source.clone());
    diagnostics.current_trailer_confidence = current_resolution
        .pointer
        .as_ref()
        .map(|pointer| pointer.confidence.clone());
    diagnostics.owned_trailers = owned_collection.owned_ids.len();
    let trailers = build_inventory(
        content,
        &owned_collection.owned_ids,
        active_trailer_id.as_deref(),
        &trailer_blocks,
        &player_vehicle_slots.assignments,
    );

    ParsedTrailerSave {
        active_trailer_id,
        current_trailer_pointer: current_resolution.pointer,
        current_trailer_diagnostics: current_resolution.diagnostics,
        player_id,
        trailer_order,
        trailers,
        player_vehicle_slots: player_vehicle_slots.slots,
        player_vehicle_assignments: player_vehicle_slots.assignments,
        diagnostics,
        unit_ids,
        unit_blocks: blocks_by_id,
        trailer_blocks,
    }
}

pub fn resolve_current_trailer_pointer(
    parsed: &ParsedTrailerSave,
) -> Result<CurrentTrailerPointer, String> {
    parsed
        .current_trailer_pointer
        .clone()
        .ok_or_else(|| "active_trailer_not_found".to_string())
}

fn resolve_current_trailer_pointer_from_parts(
    player_block: Option<&UnitBlock>,
    blocks_by_id: &HashMap<String, UnitBlock>,
    trailer_blocks: &HashMap<String, UnitBlock>,
    trailer_order: &[String],
    player_vehicle_assignments: &HashMap<String, PlayerTrailerSlotAssignment>,
) -> CurrentTrailerResolution {
    let mut diagnostics = CurrentTrailerPointerDiagnostics {
        player_found: player_block.is_some(),
        ..CurrentTrailerPointerDiagnostics::default()
    };
    let Some(player_block) = player_block else {
        return CurrentTrailerResolution {
            pointer: None,
            diagnostics,
        };
    };

    diagnostics.my_trailer_raw = extract_field_value(&player_block.raw_block, "my_trailer");
    diagnostics.assigned_vehicles_raw =
        extract_field_value(&player_block.raw_block, "assigned_vehicles");
    diagnostics.assigned_trailer_raw =
        extract_field_value(&player_block.raw_block, "assigned_trailer");

    if let Some(assigned_vehicles_raw) = diagnostics.assigned_vehicles_raw.clone() {
        if !is_null_ref(&assigned_vehicles_raw) {
            if let Some(player_vehicle_block) = find_unit_block_by_id(
                blocks_by_id,
                &assigned_vehicles_raw,
                Some("player_vehicles"),
            ) {
                diagnostics.assigned_vehicles_unit_found = true;
                diagnostics.assigned_vehicles_trailer_raw =
                    extract_field_value(&player_vehicle_block.raw_block, "trailer");
                if let Some(trailer_raw) = diagnostics.assigned_vehicles_trailer_raw.clone() {
                    if !is_null_ref(&trailer_raw) {
                        if let Some(block) = find_trailer_block_by_id(trailer_blocks, &trailer_raw)
                        {
                            diagnostics.assigned_vehicles_trailer_block_found = true;
                            return resolved_current_trailer_pointer(
                                CurrentTrailerPointer {
                                    kind: CurrentTrailerPointerKind::PlayerAssignedVehicles,
                                    trailer_id: block.id.clone(),
                                    owner_unit_id: player_vehicle_block.id.clone(),
                                    field_name: "trailer".to_string(),
                                    referenced_player_vehicle_unit_id: Some(
                                        player_vehicle_block.id.clone(),
                                    ),
                                    source: "player.assigned_vehicles".to_string(),
                                    confidence: "high".to_string(),
                                    writable: true,
                                },
                                diagnostics,
                            );
                        }
                    }
                }
            }
        }
    }

    if let Some(assigned_trailer_raw) = diagnostics.assigned_trailer_raw.clone() {
        if !is_null_ref(&assigned_trailer_raw) {
            if let Some(block) = find_trailer_block_by_id(trailer_blocks, &assigned_trailer_raw) {
                diagnostics.assigned_trailer_block_found = true;
                return resolved_current_trailer_pointer(
                    CurrentTrailerPointer {
                        kind: CurrentTrailerPointerKind::PlayerAssignedTrailer,
                        trailer_id: block.id.clone(),
                        owner_unit_id: player_block.id.clone(),
                        field_name: "assigned_trailer".to_string(),
                        referenced_player_vehicle_unit_id: None,
                        source: "player.assigned_trailer".to_string(),
                        confidence: "medium".to_string(),
                        writable: true,
                    },
                    diagnostics,
                );
            }
        }
    }

    if let Some(my_trailer_raw) = diagnostics.my_trailer_raw.clone() {
        if !is_null_ref(&my_trailer_raw) {
            if let Some(block) = find_trailer_block_by_id(trailer_blocks, &my_trailer_raw) {
                diagnostics.my_trailer_block_found = true;
                return resolved_current_trailer_pointer(
                    CurrentTrailerPointer {
                        kind: CurrentTrailerPointerKind::PlayerMyTrailer,
                        trailer_id: block.id.clone(),
                        owner_unit_id: player_block.id.clone(),
                        field_name: "my_trailer".to_string(),
                        referenced_player_vehicle_unit_id: None,
                        source: "player.my_trailer".to_string(),
                        confidence: "medium".to_string(),
                        writable: true,
                    },
                    diagnostics,
                );
            }
        }
    }

    let mut slots = player_vehicle_assignments.values().collect::<Vec<_>>();
    slots.sort_by_key(|slot| {
        (
            slot.slot_index.unwrap_or(usize::MAX),
            slot.slot_id.to_ascii_lowercase(),
        )
    });
    for slot in slots {
        let Some(trailer_id) = slot.trailer_id.as_deref() else {
            continue;
        };
        if let Some(block) = find_trailer_block_by_id(trailer_blocks, trailer_id) {
            diagnostics.fallback_player_vehicle_unit_id = Some(slot.slot_id.clone());
            diagnostics.fallback_player_vehicle_trailer_raw = Some(trailer_id.to_string());
            return resolved_current_trailer_pointer(
                CurrentTrailerPointer {
                    kind: CurrentTrailerPointerKind::FallbackPlayerVehicles,
                    trailer_id: block.id.clone(),
                    owner_unit_id: slot.slot_id.clone(),
                    field_name: "trailer".to_string(),
                    referenced_player_vehicle_unit_id: Some(slot.slot_id.clone()),
                    source: "fallback:first_player_vehicles_trailer".to_string(),
                    confidence: "low".to_string(),
                    writable: true,
                },
                diagnostics,
            );
        }
    }

    for trailer_id in trailer_order {
        if let Some(block) = find_trailer_block_by_id(trailer_blocks, trailer_id) {
            diagnostics.fallback_first_owned_trailer_raw = Some(trailer_id.to_string());
            return resolved_current_trailer_pointer(
                CurrentTrailerPointer {
                    kind: CurrentTrailerPointerKind::FallbackFirstOwnedTrailer,
                    trailer_id: block.id.clone(),
                    owner_unit_id: player_block.id.clone(),
                    field_name: "trailers[0]".to_string(),
                    referenced_player_vehicle_unit_id: None,
                    source: "fallback:first_owned_trailer".to_string(),
                    confidence: "low".to_string(),
                    writable: false,
                },
                diagnostics,
            );
        }
    }

    CurrentTrailerResolution {
        pointer: None,
        diagnostics,
    }
}

fn resolved_current_trailer_pointer(
    pointer: CurrentTrailerPointer,
    mut diagnostics: CurrentTrailerPointerDiagnostics,
) -> CurrentTrailerResolution {
    diagnostics.current_trailer_pointer_kind = Some(pointer.kind.clone());
    diagnostics.current_trailer_id = Some(pointer.trailer_id.clone());
    diagnostics.current_trailer_source = Some(pointer.source.clone());
    diagnostics.current_trailer_confidence = Some(pointer.confidence.clone());
    CurrentTrailerResolution {
        pointer: Some(pointer),
        diagnostics,
    }
}

struct PlayerTrailerSlotScan {
    slots: Vec<PlayerTrailerSlotAssignment>,
    assignments: HashMap<String, PlayerTrailerSlotAssignment>,
}

fn parse_player_vehicle_slots(
    player_block: Option<&UnitBlock>,
    unit_blocks: &[UnitBlock],
) -> PlayerTrailerSlotScan {
    let mut slots = Vec::new();
    let mut assignments = HashMap::new();
    let mut player_vehicle_refs = player_block
        .map(|block| extract_array_entries(&block.raw_block, "vehicles"))
        .unwrap_or_default();
    if let Some(raw) =
        player_block.and_then(|block| extract_field_value(&block.raw_block, "assigned_vehicles"))
    {
        if !is_null_ref(&raw) {
            player_vehicle_refs.push((usize::MAX - 1, raw));
        }
    }
    player_vehicle_refs.sort_by_key(|(index, _)| *index);
    player_vehicle_refs.dedup_by(|(_, left), (_, right)| {
        normalize_sii_unit_id(left) == normalize_sii_unit_id(right)
    });

    for (index, slot_id) in player_vehicle_refs {
        let Some(block) =
            find_unit_block_by_id_in_slice(unit_blocks, &slot_id, Some("player_vehicles"))
        else {
            continue;
        };
        let trailer_id =
            extract_field_value(&block.raw_block, "trailer").filter(|value| !is_null_ref(value));
        let assignment = PlayerTrailerSlotAssignment {
            slot_id: block.id.clone(),
            slot_id_normalized: normalize_sii_unit_id(&block.id),
            slot_index: if index == usize::MAX - 1 {
                None
            } else {
                Some(index)
            },
            trailer_id: trailer_id.clone(),
            trailer_id_normalized: trailer_id.as_deref().map(normalize_sii_unit_id),
        };
        if let Some(normalized) = assignment.trailer_id_normalized.clone() {
            assignments.insert(normalized, assignment.clone());
        }
        slots.push(assignment);
    }

    PlayerTrailerSlotScan { slots, assignments }
}

fn collect_owned_player_trailer_ids(
    trailer_order: &[String],
    active_trailer_id: Option<&str>,
    trailer_blocks: &HashMap<String, UnitBlock>,
) -> OwnedTrailerCollection {
    let mut diagnostics = OwnedTrailerDiagnostics {
        total_trailer_blocks: trailer_blocks.len(),
        player_trailers_array_count: trailer_order.len(),
        ..OwnedTrailerDiagnostics::default()
    };
    let mut owned = Vec::new();
    let mut seen = HashSet::new();

    for id in trailer_order {
        if find_trailer_block_by_id(trailer_blocks, id).is_some() {
            diagnostics.player_trailer_refs_with_blocks += 1;
        } else {
            diagnostics
                .player_trailer_reference_missing_blocks
                .push(id.to_string());
        }
        add_owned_id(id, trailer_blocks, &mut owned, &mut seen, &mut diagnostics);
    }

    if let Some(active) = active_trailer_id {
        if !seen.contains(&normalize_sii_unit_id(active)) {
            add_owned_id(
                active,
                trailer_blocks,
                &mut owned,
                &mut seen,
                &mut diagnostics,
            );
        }
    }

    OwnedTrailerCollection {
        owned_ids: owned,
        diagnostics,
    }
}

fn add_owned_id(
    id: &str,
    trailer_blocks: &HashMap<String, UnitBlock>,
    owned: &mut Vec<String>,
    seen: &mut HashSet<String>,
    diagnostics: &mut OwnedTrailerDiagnostics,
) {
    if is_null_ref(id) {
        return;
    }
    let seen_key = normalize_sii_unit_id(id);
    if !seen.insert(seen_key) {
        diagnostics.excluded_duplicates += 1;
        return;
    }
    let Some(block) = find_trailer_block_by_id(trailer_blocks, id) else {
        diagnostics.excluded_invalid += 1;
        owned.push(id.to_string());
        return;
    };
    owned.push(block.id.clone());
}

fn build_inventory(
    content: &str,
    owned_order: &[String],
    active_trailer_id: Option<&str>,
    trailer_blocks: &HashMap<String, UnitBlock>,
    player_vehicle_assignments: &HashMap<String, PlayerTrailerSlotAssignment>,
) -> Vec<TrailerInventoryItem> {
    let parsed_trailers = parse_trailers_from_sii(content);
    let defs = parse_trailer_defs_from_sii(content);
    let by_id = parsed_trailers
        .iter()
        .map(|trailer| (normalize_sii_unit_id(&trailer.trailer_id), trailer))
        .collect::<HashMap<_, _>>();

    owned_order
        .iter()
        .enumerate()
        .map(|(index, trailer_id)| {
            let normalized_id = normalize_sii_unit_id(trailer_id);
            let block = find_trailer_block_by_id(trailer_blocks, trailer_id);
            by_id
                .get(&normalized_id)
                .map(|parsed| {
                    build_inventory_item(
                        index + 1,
                        parsed,
                        active_trailer_id,
                        block,
                        player_vehicle_assignments.get(&normalized_id),
                        "player.trailers[]",
                    )
                })
                .unwrap_or_else(|| {
                    build_missing_inventory_item(
                        index + 1,
                        block.map(|item| item.id.as_str()).unwrap_or(trailer_id),
                        active_trailer_id,
                        block,
                        player_vehicle_assignments.get(&normalized_id),
                    )
                })
        })
        .map(|mut item| {
            if let Some(def) = defs.get(&item.trailer_id) {
                item.technical_details["definition"] = json!(def.id);
            }
            item
        })
        .collect()
}

fn build_inventory_item(
    display_index: usize,
    parsed: &TrailerData,
    active_trailer_id: Option<&str>,
    block: Option<&UnitBlock>,
    assignment: Option<&PlayerTrailerSlotAssignment>,
    source: &str,
) -> TrailerInventoryItem {
    let is_active = active_trailer_id
        .map(|id| normalize_sii_unit_id(id) == normalize_sii_unit_id(&parsed.trailer_id))
        .unwrap_or(false);
    let display_license_plate = parsed
        .license_plate
        .as_deref()
        .map(license_plate_display_value)
        .filter(|value| !value.trim().is_empty());
    let country_code = parsed
        .license_plate
        .as_deref()
        .and_then(license_plate_country_code);
    let brand = non_empty(parsed.brand.as_deref().unwrap_or_default())
        .or_else(|| trailer_family_from_definition(&parsed.trailer_definition).0);
    let model = non_empty(parsed.model.as_deref().unwrap_or_default())
        .or_else(|| trailer_family_from_definition(&parsed.trailer_definition).1);
    let display_name = [brand.as_deref(), model.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    let wear = trailer_wear(parsed);

    TrailerInventoryItem {
        id: parsed.trailer_id.clone(),
        trailer_id: parsed.trailer_id.clone(),
        unit_id: parsed.trailer_id.clone(),
        nameless_id: parsed.trailer_id.clone(),
        display_index,
        display_name: if display_name.trim().is_empty() {
            format!("Trailer {}", display_index)
        } else {
            display_name
        },
        brand,
        model,
        raw_license_plate: parsed.license_plate.clone(),
        display_license_plate: display_license_plate.clone(),
        license_plate: display_license_plate,
        garage_city: parsed.assigned_garage.as_deref().map(pretty_token_value),
        garage_country: country_code.clone(),
        garage_id: parsed.assigned_garage.clone(),
        garage_display_name: parsed.assigned_garage.as_deref().map(pretty_token_value),
        assigned_garage: parsed.assigned_garage.clone(),
        driver_label: None,
        owner_label: Some("Player".to_string()),
        assignment_label: assignment.as_ref().map(|slot| {
            format!(
                "player_vehicles {}",
                slot.slot_index
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string())
            )
        }),
        is_active,
        is_available: !is_active,
        is_switchable: !is_active,
        availability_reason: if is_active {
            Some("target_already_active".to_string())
        } else {
            None
        },
        assigned_driver_id: None,
        assigned_storage_id: assignment.map(|slot| slot.slot_id.clone()),
        source: source.to_string(),
        accessory_count: block
            .map(|item| extract_array_values(&item.raw_block, "accessories").len())
            .unwrap_or_else(|| parsed.accessories.len()),
        cargo_mass: Some(parsed.cargo_mass),
        wear,
        player_vehicle_slot_id: assignment.map(|slot| slot.slot_id.clone()),
        player_vehicle_slot_index: assignment.and_then(|slot| slot.slot_index),
        technical_details: json!({
            "trailerDefinition": parsed.trailer_definition,
            "blockFound": block.is_some(),
            "playerVehicleSlotId": assignment.map(|slot| slot.slot_id.clone()),
            "source": source,
        }),
    }
}

fn build_missing_inventory_item(
    display_index: usize,
    trailer_id: &str,
    active_trailer_id: Option<&str>,
    block: Option<&UnitBlock>,
    assignment: Option<&PlayerTrailerSlotAssignment>,
) -> TrailerInventoryItem {
    let is_active = active_trailer_id
        .map(|id| normalize_sii_unit_id(id) == normalize_sii_unit_id(trailer_id))
        .unwrap_or(false);
    TrailerInventoryItem {
        id: trailer_id.to_string(),
        trailer_id: trailer_id.to_string(),
        unit_id: trailer_id.to_string(),
        nameless_id: trailer_id.to_string(),
        display_index,
        display_name: format!("Trailer {}", display_index),
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
        owner_label: Some("Player".to_string()),
        assignment_label: assignment.as_ref().map(|slot| slot.slot_id.clone()),
        is_active,
        is_available: !is_active && block.is_some(),
        is_switchable: !is_active && block.is_some(),
        availability_reason: if is_active {
            Some("target_already_active".to_string())
        } else if block.is_none() {
            Some("target_trailer_not_found".to_string())
        } else {
            None
        },
        assigned_driver_id: None,
        assigned_storage_id: assignment.map(|slot| slot.slot_id.clone()),
        source: "player.trailers[]".to_string(),
        accessory_count: block
            .map(|item| extract_array_values(&item.raw_block, "accessories").len())
            .unwrap_or(0),
        cargo_mass: None,
        wear: None,
        player_vehicle_slot_id: assignment.map(|slot| slot.slot_id.clone()),
        player_vehicle_slot_index: assignment.and_then(|slot| slot.slot_index),
        technical_details: json!({
            "blockFound": block.is_some(),
            "source": "player.trailers[]",
        }),
    }
}

pub fn find_trailer_block_by_id<'a>(
    blocks: &'a HashMap<String, UnitBlock>,
    trailer_id: &str,
) -> Option<&'a UnitBlock> {
    let normalized = normalize_sii_unit_id(trailer_id);
    blocks
        .values()
        .find(|block| normalize_sii_unit_id(&block.id) == normalized)
}

pub fn find_unit_block_by_id<'a>(
    blocks: &'a HashMap<String, UnitBlock>,
    unit_id: &str,
    unit_type: Option<&str>,
) -> Option<&'a UnitBlock> {
    let normalized = normalize_sii_unit_id(unit_id);
    blocks.values().find(|block| {
        normalize_sii_unit_id(&block.id) == normalized
            && unit_type
                .map(|expected| block.unit_type.eq_ignore_ascii_case(expected))
                .unwrap_or(true)
    })
}

fn find_unit_block_by_id_in_slice<'a>(
    blocks: &'a [UnitBlock],
    unit_id: &str,
    unit_type: Option<&str>,
) -> Option<&'a UnitBlock> {
    let normalized = normalize_sii_unit_id(unit_id);
    blocks.iter().find(|block| {
        normalize_sii_unit_id(&block.id) == normalized
            && unit_type
                .map(|expected| block.unit_type.eq_ignore_ascii_case(expected))
                .unwrap_or(true)
    })
}

fn trailer_wear(parsed: &TrailerData) -> Option<f32> {
    let mut values = vec![
        parsed.wear_float.unwrap_or(0.0),
        parsed.body_wear_unfixable,
        parsed.chassis_wear,
        parsed.chassis_wear_unfixable,
    ];
    values.extend(parsed.wheels_float.clone().unwrap_or_default());
    values.extend(parsed.wheels_wear_unfixable.clone());
    values.retain(|value| value.is_finite());
    if values.is_empty() {
        None
    } else {
        Some(values.into_iter().fold(0.0_f32, f32::max))
    }
}

fn trailer_family_from_definition(definition: &str) -> (Option<String>, Option<String>) {
    let tokens = definition
        .replace('/', ".")
        .split('.')
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    let brand = tokens.get(1).map(|value| pretty_token_value(value));
    let model = tokens.get(2).map(|value| pretty_token_value(value));
    (brand, model)
}

fn license_plate_display_value(raw: &str) -> String {
    license_plate_display_text(raw)
}

fn license_plate_country_code(raw: &str) -> Option<String> {
    raw.split_once('|')
        .map(|(_, country)| country.trim().to_string())
        .filter(|country| !country.is_empty())
}

fn pretty_token_value(value: &str) -> String {
    let without_prefix = value
        .trim()
        .trim_matches('"')
        .split('.')
        .last()
        .unwrap_or(value)
        .replace('_', " ")
        .replace('-', " ");
    without_prefix
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
