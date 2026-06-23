use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use regex::Regex;

use crate::models::trucks::ParsedTruck;
use crate::shared::sii_parser::{get_player_id, parse_trucks_from_sii};

use super::models::{
    CurrentTruckPointer, CurrentTruckPointerDiagnostics, CurrentTruckPointerKind,
    DriverDisplayInfo, DriverParserDiagnostics, GarageCapacity, GarageSlotAssignment,
    OwnedTruckDiagnostics, OwnedTruckSource, PlayerVehicleSlotAssignment, TruckAssignment,
    TruckGraph, TruckInventoryItem,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitBlock {
    pub unit_type: String,
    pub id: String,
    pub start_line: usize,
    pub end_line: usize,
    pub raw_block: String,
}

#[derive(Debug, Clone)]
pub struct ParsedTruckSave {
    pub active_truck_id: Option<String>,
    pub current_truck_pointer: Option<CurrentTruckPointer>,
    pub current_truck_diagnostics: CurrentTruckPointerDiagnostics,
    pub player_id: Option<String>,
    pub truck_order: Vec<String>,
    pub trucks: Vec<TruckInventoryItem>,
    pub truck_graphs: HashMap<String, TruckGraph>,
    pub player_vehicle_slots: Vec<PlayerVehicleSlotAssignment>,
    pub player_vehicle_assignments: HashMap<String, PlayerVehicleSlotAssignment>,
    pub garage_assignments: HashMap<String, GarageSlotAssignment>,
    pub garages: Vec<GarageCapacity>,
    pub driver_infos: HashMap<String, DriverDisplayInfo>,
    pub driver_diagnostics: DriverParserDiagnostics,
    pub truck_assignments: HashMap<String, TruckAssignment>,
    pub diagnostics: OwnedTruckDiagnostics,
    pub unit_ids: HashSet<String>,
    pub unit_blocks: HashMap<String, UnitBlock>,
}

pub fn parse_truck_save(content: &str) -> ParsedTruckSave {
    let unit_blocks = parse_unit_blocks(content);
    let unit_ids = unit_blocks
        .iter()
        .map(|block| block.id.clone())
        .collect::<HashSet<_>>();
    let blocks_by_id = unit_blocks
        .iter()
        .map(|block| (block.id.clone(), block.clone()))
        .collect::<HashMap<_, _>>();
    let player_id = get_player_id(content);
    let player_block = player_id
        .as_ref()
        .and_then(|id| blocks_by_id.get(id))
        .cloned();
    let truck_order = player_block
        .as_ref()
        .map(|block| extract_array_values(&block.raw_block, "trucks"))
        .unwrap_or_default()
        .into_iter()
        .filter(|value| !is_null_ref(value))
        .collect::<Vec<_>>();
    let player_vehicle_scan = parse_player_vehicle_slots(player_block.as_ref(), &unit_blocks);
    let garage_scan = parse_garages(&unit_blocks);
    let mut driver_scan = parse_driver_infos(&unit_blocks);
    driver_scan.diagnostics.unresolved_driver_ids = garage_scan
        .assignments
        .values()
        .filter_map(|assignment| {
            let normalized = assignment.driver_id_normalized.as_deref()?;
            if driver_scan.infos.contains_key(normalized) {
                None
            } else {
                assignment.driver_id.clone()
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let driver_infos = driver_scan.infos;
    let accessory_blocks = unit_blocks
        .iter()
        .filter(|block| {
            block.unit_type == "vehicle_accessory" || block.unit_type == "vehicle_addon_accessory"
        })
        .map(|block| (block.id.clone(), block.clone()))
        .collect::<HashMap<_, _>>();
    let truck_graphs = unit_blocks
        .iter()
        .filter(|block| block.unit_type == "vehicle")
        .map(|block| {
            let graph = build_truck_graph(block, &accessory_blocks);
            (graph.vehicle_id.clone(), graph)
        })
        .collect::<HashMap<_, _>>();
    let current_truck_resolution = resolve_current_truck_pointer_from_parts(
        player_block.as_ref(),
        &blocks_by_id,
        &truck_graphs,
        &truck_order,
        &player_vehicle_scan.assignments,
    );
    let active_truck_id = current_truck_resolution
        .pointer
        .as_ref()
        .map(|pointer| pointer.truck_id.clone());
    let owned_collection = collect_owned_player_truck_ids(
        &unit_blocks,
        &truck_order,
        &truck_graphs,
    );
    let mut owned_diagnostics = owned_collection.diagnostics;
    owned_diagnostics.current_truck_pointer_kind = current_truck_resolution
        .pointer
        .as_ref()
        .map(|pointer| pointer.kind.clone());
    owned_diagnostics.current_truck_id = current_truck_resolution
        .pointer
        .as_ref()
        .map(|pointer| pointer.truck_id.clone());
    owned_diagnostics.assigned_vehicles_unit_id = current_truck_resolution
        .pointer
        .as_ref()
        .and_then(|pointer| pointer.referenced_player_vehicle_unit_id.clone());
    owned_diagnostics.current_truck_pointer = current_truck_resolution.diagnostics.clone();
    owned_diagnostics.current_truck_source = current_truck_resolution
        .pointer
        .as_ref()
        .map(|pointer| pointer.source.clone());
    owned_diagnostics.current_truck_confidence = current_truck_resolution
        .pointer
        .as_ref()
        .map(|pointer| pointer.confidence.clone());
    crate::dev_log!(
        "[truck_change] owned truck collection completed vehicle_blocks={} candidate_trucks={} owned_trucks={}",
        owned_diagnostics.total_vehicle_blocks,
        owned_diagnostics.candidate_trucks,
        owned_diagnostics.owned_trucks
    );
    crate::dev_log!(
        "[truck_change] excluded non-owned vehicle blocks: {}",
        owned_diagnostics.excluded_unreferenced
            + owned_diagnostics.excluded_job_vehicles
            + owned_diagnostics.excluded_invalid
    );
    let trucks = build_inventory(
        content,
        &owned_collection.owned_ids,
        active_truck_id.as_deref(),
        &truck_graphs,
        &garage_scan.assignments,
        &player_vehicle_scan.assignments,
        &driver_infos,
    );
    let truck_assignments = build_truck_assignments(
        &trucks,
        &garage_scan.assignments,
        &driver_infos,
        active_truck_id.as_deref(),
    );

    ParsedTruckSave {
        active_truck_id,
        current_truck_pointer: current_truck_resolution.pointer,
        current_truck_diagnostics: current_truck_resolution.diagnostics,
        player_id,
        truck_order,
        trucks,
        truck_graphs,
        player_vehicle_slots: player_vehicle_scan.slots,
        player_vehicle_assignments: player_vehicle_scan.assignments,
        garage_assignments: garage_scan.assignments,
        garages: garage_scan.garages,
        driver_infos,
        driver_diagnostics: driver_scan.diagnostics,
        truck_assignments,
        diagnostics: owned_diagnostics,
        unit_ids,
        unit_blocks: blocks_by_id,
    }
}

pub fn parse_unit_blocks(content: &str) -> Vec<UnitBlock> {
    let header_re = Regex::new(r"^([A-Za-z0-9_]+)\s*:\s*([^\s{]+)\s*\{").expect("valid unit regex");
    let lines = content.lines().collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();
        let Some(captures) = header_re.captures(trimmed) else {
            index += 1;
            continue;
        };

        let unit_type = captures
            .get(1)
            .map(|value| value.as_str().to_string())
            .unwrap_or_default();
        let id = captures
            .get(2)
            .map(|value| value.as_str().trim_end_matches('{').to_string())
            .unwrap_or_default();
        if unit_type.is_empty() || id.is_empty() {
            index += 1;
            continue;
        }

        let start = index;
        let mut end = index;
        let mut depth =
            lines[index].matches('{').count() as i32 - lines[index].matches('}').count() as i32;
        while end + 1 < lines.len() && depth > 0 {
            end += 1;
            depth += lines[end].matches('{').count() as i32;
            depth -= lines[end].matches('}').count() as i32;
        }

        let raw_block = lines[start..=end].join("\n");
        blocks.push(UnitBlock {
            unit_type,
            id,
            start_line: start,
            end_line: end,
            raw_block,
        });
        index = end + 1;
    }

    blocks
}

pub fn extract_field_value(raw_block: &str, field: &str) -> Option<String> {
    let prefix = format!("{}:", field);
    raw_block.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(&prefix) {
            return None;
        }
        trimmed
            .split_once(':')
            .map(|(_, value)| normalize_value(value.trim()))
            .filter(|value| !value.is_empty())
    })
}

pub fn extract_array_values(raw_block: &str, field: &str) -> Vec<String> {
    extract_array_entries(raw_block, field)
        .into_iter()
        .map(|(_, value)| value)
        .collect()
}

pub fn extract_array_entries(raw_block: &str, field: &str) -> Vec<(usize, String)> {
    let item_re = Regex::new(&format!(r"^{}\[(\d+)\]\s*:", regex::escape(field)))
        .expect("valid array field regex");
    let mut values = raw_block
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let index = item_re
                .captures(trimmed)
                .and_then(|captures| captures.get(1))
                .and_then(|value| value.as_str().parse::<usize>().ok())?;
            let value = trimmed
                .split_once(':')
                .map(|(_, value)| normalize_value(value.trim()))?;
            if value.is_empty() {
                None
            } else {
                Some((index, value))
            }
        })
        .collect::<Vec<_>>();
    values.sort_by_key(|(index, _)| *index);
    values
}

pub fn normalize_value(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(',')
        .trim_matches('"')
        .trim()
        .to_string()
}

pub fn normalize_sii_unit_id(value: &str) -> String {
    let mut normalized = normalize_value(value)
        .trim_end_matches(';')
        .trim_end_matches('{')
        .trim_end_matches('}')
        .trim()
        .to_ascii_lowercase();
    while normalized.ends_with(',') || normalized.ends_with(';') {
        normalized.pop();
        normalized = normalized.trim().to_string();
    }
    normalized
}

pub fn is_null_ref(value: &str) -> bool {
    let normalized = value.trim();
    normalized.is_empty()
        || normalized.eq_ignore_ascii_case("null")
        || normalized.eq_ignore_ascii_case("nil")
}

struct CurrentTruckResolution {
    pointer: Option<CurrentTruckPointer>,
    diagnostics: CurrentTruckPointerDiagnostics,
}

pub fn resolve_current_truck_pointer(
    parsed: &ParsedTruckSave,
) -> Result<CurrentTruckPointer, String> {
    parsed
        .current_truck_pointer
        .clone()
        .ok_or_else(|| "current_truck_unresolved".to_string())
}

fn resolve_current_truck_pointer_from_parts(
    player_block: Option<&UnitBlock>,
    blocks_by_id: &HashMap<String, UnitBlock>,
    graphs: &HashMap<String, TruckGraph>,
    truck_order: &[String],
    player_vehicle_assignments: &HashMap<String, PlayerVehicleSlotAssignment>,
) -> CurrentTruckResolution {
    let mut diagnostics = CurrentTruckPointerDiagnostics {
        player_found: player_block.is_some(),
        ..CurrentTruckPointerDiagnostics::default()
    };
    let Some(player_block) = player_block else {
        return CurrentTruckResolution {
            pointer: None,
            diagnostics,
        };
    };

    diagnostics.my_truck_raw = extract_field_value(&player_block.raw_block, "my_truck");
    diagnostics.assigned_vehicles_raw =
        extract_field_value(&player_block.raw_block, "assigned_vehicles");
    diagnostics.assigned_truck_raw = extract_field_value(&player_block.raw_block, "assigned_truck");

    if let Some(assigned_vehicles_raw) = diagnostics.assigned_vehicles_raw.clone() {
        if !is_null_ref(&assigned_vehicles_raw) {
            if let Some(player_vehicle_block) = find_unit_block_by_id(
                blocks_by_id,
                &assigned_vehicles_raw,
                Some("player_vehicles"),
            ) {
                diagnostics.assigned_vehicles_unit_found = true;
                diagnostics.assigned_vehicles_vehicle_raw =
                    extract_field_value(&player_vehicle_block.raw_block, "vehicle");
                if let Some(vehicle_raw) = diagnostics.assigned_vehicles_vehicle_raw.clone() {
                    if !is_null_ref(&vehicle_raw) {
                        if let Some(graph) = find_truck_graph_by_id(graphs, &vehicle_raw) {
                            diagnostics.assigned_vehicles_vehicle_block_found = true;
                            return resolved_current_truck_pointer(
                                CurrentTruckPointer {
                                    kind: CurrentTruckPointerKind::PlayerAssignedVehicles,
                                    truck_id: graph.vehicle_id.clone(),
                                    owner_unit_id: player_vehicle_block.id.clone(),
                                    field_name: "vehicle".to_string(),
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

    if let Some(assigned_truck_raw) = diagnostics.assigned_truck_raw.clone() {
        if !is_null_ref(&assigned_truck_raw) {
            if let Some(graph) = find_truck_graph_by_id(graphs, &assigned_truck_raw) {
                diagnostics.assigned_truck_vehicle_block_found = true;
                return resolved_current_truck_pointer(
                    CurrentTruckPointer {
                        kind: CurrentTruckPointerKind::PlayerAssignedTruck,
                        truck_id: graph.vehicle_id.clone(),
                        owner_unit_id: player_block.id.clone(),
                        field_name: "assigned_truck".to_string(),
                        referenced_player_vehicle_unit_id: None,
                        source: "player.assigned_truck".to_string(),
                        confidence: "medium".to_string(),
                        writable: true,
                    },
                    diagnostics,
                );
            }
        }
    }

    if let Some(my_truck_raw) = diagnostics.my_truck_raw.clone() {
        if !is_null_ref(&my_truck_raw) {
            if let Some(graph) = find_truck_graph_by_id(graphs, &my_truck_raw) {
                diagnostics.my_truck_vehicle_block_found = true;
                return resolved_current_truck_pointer(
                    CurrentTruckPointer {
                        kind: CurrentTruckPointerKind::PlayerMyTruck,
                        truck_id: graph.vehicle_id.clone(),
                        owner_unit_id: player_block.id.clone(),
                        field_name: "my_truck".to_string(),
                        referenced_player_vehicle_unit_id: None,
                        source: "player.my_truck".to_string(),
                        confidence: "medium".to_string(),
                        writable: true,
                    },
                    diagnostics,
                );
            }
        }
    }

    let mut player_vehicle_slots = player_vehicle_assignments.values().collect::<Vec<_>>();
    player_vehicle_slots.sort_by_key(|slot| {
        (
            slot.slot_index.unwrap_or(usize::MAX),
            slot.slot_id.to_ascii_lowercase(),
        )
    });
    for slot in player_vehicle_slots {
        let Some(truck_id) = slot.truck_id.as_deref() else {
            continue;
        };
        if let Some(graph) = find_truck_graph_by_id(graphs, truck_id) {
            diagnostics.fallback_player_vehicle_unit_id = Some(slot.slot_id.clone());
            diagnostics.fallback_player_vehicle_vehicle_raw = Some(truck_id.to_string());
            return resolved_current_truck_pointer(
                CurrentTruckPointer {
                    kind: CurrentTruckPointerKind::FallbackPlayerVehicles,
                    truck_id: graph.vehicle_id.clone(),
                    owner_unit_id: slot.slot_id.clone(),
                    field_name: "vehicle".to_string(),
                    referenced_player_vehicle_unit_id: Some(slot.slot_id.clone()),
                    source: "fallback:first_player_vehicles_vehicle".to_string(),
                    confidence: "low".to_string(),
                    writable: true,
                },
                diagnostics,
            );
        }
    }

    for truck_id in truck_order {
        if let Some(graph) = find_truck_graph_by_id(graphs, truck_id) {
            diagnostics.fallback_first_owned_truck_raw = Some(truck_id.to_string());
            return resolved_current_truck_pointer(
                CurrentTruckPointer {
                    kind: CurrentTruckPointerKind::FallbackFirstOwnedTruck,
                    truck_id: graph.vehicle_id.clone(),
                    owner_unit_id: player_block.id.clone(),
                    field_name: "trucks[0]".to_string(),
                    referenced_player_vehicle_unit_id: None,
                    source: "fallback:first_owned_truck".to_string(),
                    confidence: "low".to_string(),
                    writable: false,
                },
                diagnostics,
            );
        }
    }

    CurrentTruckResolution {
        pointer: None,
        diagnostics,
    }
}

fn resolved_current_truck_pointer(
    pointer: CurrentTruckPointer,
    mut diagnostics: CurrentTruckPointerDiagnostics,
) -> CurrentTruckResolution {
    diagnostics.current_truck_pointer_kind = Some(pointer.kind.clone());
    diagnostics.current_truck_id = Some(pointer.truck_id.clone());
    diagnostics.current_truck_source = Some(pointer.source.clone());
    diagnostics.current_truck_confidence = Some(pointer.confidence.clone());
    CurrentTruckResolution {
        pointer: Some(pointer),
        diagnostics,
    }
}

fn find_unit_block_by_id<'a>(
    blocks_by_id: &'a HashMap<String, UnitBlock>,
    unit_id: &str,
    unit_type: Option<&str>,
) -> Option<&'a UnitBlock> {
    let normalized = normalize_sii_unit_id(unit_id);
    blocks_by_id.values().find(|block| {
        normalize_sii_unit_id(&block.id) == normalized
            && unit_type
                .map(|expected| block.unit_type.eq_ignore_ascii_case(expected))
                .unwrap_or(true)
    })
}

fn find_truck_graph_by_id<'a>(
    graphs: &'a HashMap<String, TruckGraph>,
    truck_id: &str,
) -> Option<&'a TruckGraph> {
    graphs.get(truck_id).or_else(|| {
        let normalized = normalize_sii_unit_id(truck_id);
        graphs
            .values()
            .find(|graph| normalize_sii_unit_id(&graph.vehicle_id) == normalized)
    })
}

pub fn is_valid_garage_driver_ref(value: &str) -> bool {
    let normalized = normalize_sii_unit_id(value);
    if normalized.is_empty()
        || is_null_ref(&normalized)
        || normalized.eq_ignore_ascii_case("none")
        || normalized == "0"
        || normalized.starts_with("_nameless.")
    {
        return false;
    }

    let Some(suffix) = normalized.strip_prefix("driver.") else {
        return false;
    };

    !suffix.is_empty() && suffix.chars().all(|value| value.is_ascii_digit())
}

pub fn garage_driver_ref_is_unique(
    parsed: &ParsedTruckSave,
    driver_id: &str,
    target_truck_id: &str,
) -> bool {
    if !is_valid_garage_driver_ref(driver_id) {
        return false;
    }

    let driver_id_normalized = normalize_sii_unit_id(driver_id);
    let target_truck_id_normalized = normalize_sii_unit_id(target_truck_id);
    if driver_id_normalized.is_empty() || target_truck_id_normalized.is_empty() {
        return false;
    }

    let Some(target_assignment) = parsed.garage_assignments.get(&target_truck_id_normalized) else {
        return false;
    };
    if !target_assignment.arrays_have_matching_indices {
        return false;
    }

    let mut target_slots = Vec::new();
    let mut driver_slots = Vec::new();

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

        for (index, truck_id) in vehicles.iter() {
            if is_null_ref(truck_id) {
                continue;
            }

            if normalize_sii_unit_id(truck_id) == target_truck_id_normalized {
                target_slots.push((block.id.clone(), *index));
            }
        }

        for (index, slot_driver_id) in drivers.iter() {
            if is_null_ref(slot_driver_id) {
                continue;
            }
            if normalize_sii_unit_id(slot_driver_id) != driver_id_normalized {
                continue;
            }

            let Some(slot_truck_id) = vehicles.get(index).filter(|value| !is_null_ref(value))
            else {
                return false;
            };

            driver_slots.push((
                block.id.clone(),
                *index,
                normalize_sii_unit_id(slot_truck_id),
            ));
        }
    }

    if target_slots.len() != 1 || driver_slots.len() != 1 {
        return false;
    }

    let (target_garage_id, target_slot_index) = &target_slots[0];
    let (driver_garage_id, driver_slot_index, driver_truck_id) = &driver_slots[0];

    target_garage_id == driver_garage_id
        && target_slot_index == driver_slot_index
        && driver_truck_id == &target_truck_id_normalized
}

pub fn truck_family_from_data_path(data_path: &str) -> Option<String> {
    let normalized = data_path.trim().trim_matches('"').replace('\\', "/");
    let marker = "/def/vehicle/truck/";
    let start = normalized.find(marker)? + marker.len();
    normalized[start..]
        .split('/')
        .next()
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
}

pub fn brand_model_from_family(family: &str) -> (Option<String>, Option<String>) {
    let mut parts = family.split('.');
    let brand = parts.next().map(|value| value.to_string());
    let model = parts.collect::<Vec<_>>().join(".");
    let model = if model.is_empty() { None } else { Some(model) };
    (brand, model)
}

fn build_truck_graph(
    vehicle_block: &UnitBlock,
    accessory_blocks: &HashMap<String, UnitBlock>,
) -> TruckGraph {
    let accessory_ids = extract_array_values(&vehicle_block.raw_block, "accessories")
        .into_iter()
        .filter(|value| !is_null_ref(value))
        .collect::<Vec<_>>();
    let accessories = accessory_ids
        .iter()
        .filter_map(|id| accessory_blocks.get(id))
        .map(|block| {
            let data_path = extract_field_value(&block.raw_block, "data_path");
            let references = extract_references(&block.raw_block)
                .into_iter()
                .filter(|reference| reference != &block.id)
                .collect::<Vec<_>>();
            super::models::VehicleAccessoryNode {
                id: block.id.clone(),
                unit_type: block.unit_type.clone(),
                data_path,
                raw_block: block.raw_block.clone(),
                references,
            }
        })
        .collect::<Vec<_>>();
    let referenced_unit_ids = extract_references(&vehicle_block.raw_block)
        .into_iter()
        .chain(
            accessories
                .iter()
                .flat_map(|accessory| accessory.references.iter().cloned()),
        )
        .filter(|reference| reference != &vehicle_block.id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    TruckGraph {
        vehicle_id: vehicle_block.id.clone(),
        vehicle_block: vehicle_block.raw_block.clone(),
        accessory_ids,
        accessories,
        referenced_unit_ids,
    }
}

pub fn graph_engine_data_path(graph: &TruckGraph) -> Option<String> {
    graph
        .accessories
        .iter()
        .filter_map(|accessory| accessory.data_path.as_deref())
        .find(|path| path.replace('\\', "/").contains("/engine/"))
        .map(|path| path.to_string())
}

pub fn graph_transmission_data_path(graph: &TruckGraph) -> Option<String> {
    graph
        .accessories
        .iter()
        .filter_map(|accessory| accessory.data_path.as_deref())
        .find(|path| path.replace('\\', "/").contains("/transmission/"))
        .map(|path| path.to_string())
}

pub fn graph_primary_family(graph: &TruckGraph) -> Option<String> {
    graph
        .accessories
        .iter()
        .filter_map(|accessory| accessory.data_path.as_deref())
        .find(|path| {
            let normalized = path.replace('\\', "/").to_ascii_lowercase();
            normalized.starts_with("/def/vehicle/truck/") && normalized.ends_with("/data.sii")
        })
        .and_then(truck_family_from_data_path)
        .or_else(|| {
            graph
                .accessories
                .iter()
                .filter_map(|accessory| accessory.data_path.as_deref())
                .find_map(truck_family_from_data_path)
        })
}

pub fn graph_dangling_accessories(graph: &TruckGraph, unit_ids: &HashSet<String>) -> Vec<String> {
    graph
        .accessory_ids
        .iter()
        .filter(|id| !unit_ids.contains(*id))
        .cloned()
        .collect()
}

fn extract_references(raw_block: &str) -> Vec<String> {
    let reference_re = Regex::new(r"_nameless\.[A-Za-z0-9._]+").expect("valid reference regex");
    let mut references = reference_re
        .find_iter(raw_block)
        .map(|value| value.as_str().to_string())
        .collect::<Vec<_>>();
    references.sort();
    references.dedup();
    references
}

struct GarageScan {
    assignments: HashMap<String, GarageSlotAssignment>,
    garages: Vec<GarageCapacity>,
}

struct PlayerVehicleScan {
    slots: Vec<PlayerVehicleSlotAssignment>,
    assignments: HashMap<String, PlayerVehicleSlotAssignment>,
}

struct DriverScan {
    infos: HashMap<String, DriverDisplayInfo>,
    diagnostics: DriverParserDiagnostics,
}

struct OwnedTruckCollection {
    owned_ids: Vec<String>,
    diagnostics: OwnedTruckDiagnostics,
}

fn parse_player_vehicle_slots(
    player_block: Option<&UnitBlock>,
    unit_blocks: &[UnitBlock],
) -> PlayerVehicleScan {
    let mut slots = Vec::new();
    let mut assignments = HashMap::new();
    let mut seen_slot_ids = HashSet::new();
    let blocks_by_id = unit_blocks
        .iter()
        .map(|block| (normalize_sii_unit_id(&block.id), block))
        .collect::<HashMap<_, _>>();

    if let Some(player_block) = player_block {
        for (index, slot_id) in extract_array_entries(&player_block.raw_block, "my_vehicles") {
            if is_null_ref(&slot_id) {
                continue;
            }
            let normalized_slot_id = normalize_sii_unit_id(&slot_id);
            let Some(slot_block) = blocks_by_id.get(&normalized_slot_id).copied() else {
                continue;
            };
            if slot_block.unit_type != "player_vehicles" {
                continue;
            }
            if !seen_slot_ids.insert(normalized_slot_id) {
                continue;
            }
            push_player_vehicle_slot(slot_block, Some(index), &mut slots, &mut assignments);
        }
    }

    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "player_vehicles")
    {
        let normalized_slot_id = normalize_sii_unit_id(&block.id);
        if !seen_slot_ids.insert(normalized_slot_id) {
            continue;
        }
        push_player_vehicle_slot(block, None, &mut slots, &mut assignments);
    }

    PlayerVehicleScan { slots, assignments }
}

fn push_player_vehicle_slot(
    block: &UnitBlock,
    slot_index: Option<usize>,
    slots: &mut Vec<PlayerVehicleSlotAssignment>,
    assignments: &mut HashMap<String, PlayerVehicleSlotAssignment>,
) {
    let truck_id = extract_field_value(&block.raw_block, "vehicle").filter(|value| !is_null_ref(value));
    let truck_id_normalized = truck_id
        .as_deref()
        .map(normalize_sii_unit_id)
        .filter(|value| !value.is_empty());
    let slot = PlayerVehicleSlotAssignment {
        slot_id: block.id.clone(),
        slot_id_normalized: normalize_sii_unit_id(&block.id),
        slot_index,
        truck_id: truck_id.clone(),
        truck_id_normalized: truck_id_normalized.clone(),
    };
    if let Some(normalized_truck_id) = truck_id_normalized {
        assignments
            .entry(normalized_truck_id)
            .or_insert_with(|| slot.clone());
    }
    slots.push(slot);
}

fn parse_garages(unit_blocks: &[UnitBlock]) -> GarageScan {
    let mut assignments = HashMap::new();
    let mut garages = Vec::new();

    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "garage")
    {
        let vehicles = extract_array_entries(&block.raw_block, "vehicles");
        let drivers = extract_array_entries(&block.raw_block, "drivers");
        let vehicles_by_index = vehicles.iter().cloned().collect::<BTreeMap<_, _>>();
        let drivers_by_index = drivers.iter().cloned().collect::<BTreeMap<_, _>>();
        let declared_vehicle_count = extract_field_value(&block.raw_block, "vehicles")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let declared_driver_count = extract_field_value(&block.raw_block, "drivers")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let indexed_vehicle_count = vehicles
            .iter()
            .map(|(index, _)| index + 1)
            .max()
            .unwrap_or(0);
        let indexed_driver_count = drivers
            .iter()
            .map(|(index, _)| index + 1)
            .max()
            .unwrap_or(0);
        let garage_vehicle_count = declared_vehicle_count.max(indexed_vehicle_count);
        let garage_driver_count = declared_driver_count.max(indexed_driver_count);
        let arrays_have_matching_indices = vehicles_by_index
            .iter()
            .filter(|(_, truck_id)| !is_null_ref(truck_id))
            .all(|(index, _)| drivers_by_index.contains_key(index))
            && drivers_by_index
                .iter()
                .filter(|(_, driver_id)| !is_null_ref(driver_id))
                .all(|(index, _)| vehicles_by_index.contains_key(index));
        let garage_display_name = garage_display_name(block);
        let country_code = extract_first_existing_field(
            &block.raw_block,
            &["country", "country_code", "country_token"],
        );
        let country_display_name = country_code.as_deref().map(pretty_token_value);
        let mut occupied = 0usize;
        let mut free = 0usize;

        for index in 0..garage_vehicle_count {
            let Some(truck_id) = vehicles_by_index.get(&index) else {
                free += 1;
                continue;
            };
            if is_null_ref(truck_id) {
                free += 1;
                continue;
            }
            occupied += 1;
            let driver_id = drivers_by_index
                .get(&index)
                .filter(|value| !is_null_ref(value))
                .cloned();
            let truck_id_normalized = normalize_sii_unit_id(truck_id);
            let driver_id_normalized = driver_id
                .as_deref()
                .map(normalize_sii_unit_id)
                .filter(|value| !value.is_empty());
            assignments.insert(
                truck_id_normalized.clone(),
                GarageSlotAssignment {
                    garage_id: block.id.clone(),
                    garage_display_name: garage_display_name.clone(),
                    country_code: country_code.clone(),
                    country_display_name: country_display_name.clone(),
                    slot_index: index,
                    truck_id: truck_id.clone(),
                    truck_id_normalized,
                    driver_id,
                    driver_id_normalized,
                    garage_vehicle_count,
                    garage_driver_count,
                    arrays_have_matching_indices,
                },
            );
        }

        if garage_vehicle_count == 0 {
            free = declared_vehicle_count;
        }

        garages.push(GarageCapacity {
            garage_id: block.id.clone(),
            garage_display_name: garage_display_name.clone(),
            total_truck_slots: garage_vehicle_count.max(occupied + free),
            occupied_truck_slots: occupied,
            free_truck_slots: free,
        });
    }

    GarageScan {
        assignments,
        garages,
    }
}

pub fn assignment_conflicts_from_blocks(unit_blocks: &[UnitBlock]) -> Vec<String> {
    let mut truck_slots = HashMap::new();
    let mut driver_slots = HashMap::new();
    let mut conflicts = Vec::new();

    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "garage")
    {
        let vehicles = extract_array_entries(&block.raw_block, "vehicles")
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let drivers = extract_array_entries(&block.raw_block, "drivers")
            .into_iter()
            .collect::<BTreeMap<_, _>>();

        for (index, truck_id) in vehicles.iter() {
            if is_null_ref(truck_id) {
                continue;
            }

            let truck_slot = format!("{}:{}", block.id, index);
            let normalized_truck_id = normalize_sii_unit_id(truck_id);
            if let Some(previous_slot) =
                truck_slots.insert(normalized_truck_id.clone(), truck_slot.clone())
            {
                conflicts.push(format!(
                    "duplicate_truck_assignment:{}:{}:{}",
                    truck_id, previous_slot, truck_slot
                ));
            }

            let Some(driver_id) = drivers.get(index).filter(|value| !is_null_ref(value)) else {
                continue;
            };

            let normalized_driver_id = normalize_sii_unit_id(driver_id);
            if let Some(previous_slot) =
                driver_slots.insert(normalized_driver_id.clone(), truck_slot.clone())
            {
                conflicts.push(format!(
                    "duplicate_driver_assignment:{}:{}:{}",
                    driver_id, previous_slot, truck_slot
                ));
            }
        }
    }

    conflicts.sort();
    conflicts.dedup();
    conflicts
}

fn parse_driver_infos(unit_blocks: &[UnitBlock]) -> DriverScan {
    let mut infos = HashMap::new();
    let mut recognized_unit_types = BTreeSet::new();
    let mut ignored_driver_like_blocks = 0usize;

    for block in unit_blocks {
        if !is_recognized_driver_block(block) {
            if is_driver_like_block(block) {
                ignored_driver_like_blocks += 1;
            }
            continue;
        }

        let first = extract_first_existing_field(
            &block.raw_block,
            &["first_name", "forename", "name", "given_name"],
        )
        .map(|value| sanitize_sii_display_text(&value))
        .filter(|value| is_readable_display_value(value, &block.id));
        let last = extract_first_existing_field(
            &block.raw_block,
            &["last_name", "surname", "family_name"],
        )
        .map(|value| sanitize_sii_display_text(&value))
        .filter(|value| is_readable_display_value(value, &block.id));
        let joined = [first.as_deref(), last.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        let display_name = if joined.trim().is_empty() {
            extract_first_existing_field(
                &block.raw_block,
                &["display_name", "localized_name", "full_name", "driver_name"],
            )
            .map(|value| sanitize_sii_display_text(&value))
            .filter(|value| is_readable_display_value(value, &block.id))
        } else {
            Some(joined)
        };
        let current_truck_reference = extract_first_existing_field_with_name(
            &block.raw_block,
            &["assigned_truck", "assigned_vehicle", "truck", "vehicle"],
        )
        .filter(|(_, value)| !is_null_ref(value));
        let current_truck_field = current_truck_reference
            .as_ref()
            .map(|(field, _)| (*field).to_string());
        let current_truck_id = current_truck_reference.map(|(_, value)| value);
        let current_truck_id_normalized = current_truck_id
            .as_deref()
            .map(normalize_sii_unit_id)
            .filter(|value| !value.is_empty());
        let normalized_id = normalize_sii_unit_id(&block.id);
        if normalized_id.is_empty() {
            ignored_driver_like_blocks += 1;
            continue;
        }

        recognized_unit_types.insert(block.unit_type.clone());
        infos.insert(
            normalized_id.clone(),
            DriverDisplayInfo {
                driver_id: block.id.clone(),
                raw_id: block.id.clone(),
                normalized_id,
                unit_type: block.unit_type.clone(),
                display_name,
                current_truck_id,
                current_truck_id_normalized,
                current_truck_field,
            },
        );
    }

    let diagnostics = DriverParserDiagnostics {
        total_units: unit_blocks.len(),
        recognized_driver_blocks: infos.len(),
        ignored_driver_like_blocks,
        recognized_unit_types: recognized_unit_types.into_iter().collect(),
        unresolved_driver_ids: Vec::new(),
    };

    DriverScan { infos, diagnostics }
}

fn is_recognized_driver_block(block: &UnitBlock) -> bool {
    matches!(
        block.unit_type.as_str(),
        "driver" | "driver_ai" | "driver_player"
    ) && has_driver_structure(block)
}

fn has_driver_structure(block: &UnitBlock) -> bool {
    let normalized_id = normalize_sii_unit_id(&block.id);
    normalized_id.starts_with("driver.")
        || normalized_id.starts_with("_nameless.")
        || extract_first_existing_field(
            &block.raw_block,
            &[
                "first_name",
                "forename",
                "name",
                "given_name",
                "last_name",
                "surname",
                "family_name",
                "display_name",
                "localized_name",
                "full_name",
                "driver_name",
                "assigned_truck",
                "truck",
            ],
        )
        .is_some()
}

fn is_driver_like_block(block: &UnitBlock) -> bool {
    let unit_type = block.unit_type.to_ascii_lowercase();
    unit_type.contains("driver")
        || unit_type == "employee"
        || extract_first_existing_field(&block.raw_block, &["driver_name"]).is_some()
}

pub fn collect_owned_player_truck_ids_from_save(parsed: &ParsedTruckSave) -> OwnedTruckDiagnostics {
    parsed.diagnostics.clone()
}

fn collect_owned_player_truck_ids(
    unit_blocks: &[UnitBlock],
    truck_order: &[String],
    graphs: &HashMap<String, TruckGraph>,
) -> OwnedTruckCollection {
    let total_vehicle_blocks = unit_blocks
        .iter()
        .filter(|block| block.unit_type == "vehicle")
        .count();
    let candidate_trucks = graphs
        .values()
        .filter(|graph| is_truck_graph(graph))
        .count();

    let mut diagnostics = OwnedTruckDiagnostics {
        total_vehicle_blocks,
        candidate_trucks,
        ..OwnedTruckDiagnostics::default()
    };
    let mut owned = Vec::new();
    let mut seen = HashSet::new();

    diagnostics.player_trucks_array_count = truck_order.len();
    for id in truck_order {
        if find_truck_graph_by_id(graphs, id).is_some() {
            diagnostics.player_truck_refs_with_vehicle_blocks += 1;
        } else {
            diagnostics
                .player_truck_reference_missing_vehicle_blocks
                .push(id.to_string());
        }
        add_owned_id(
            id,
            OwnedTruckSource::PlayerTrucksArray,
            graphs,
            &mut owned,
            &mut seen,
            &mut diagnostics,
        );
    }

    for graph in graphs.values() {
        if seen.contains(&normalize_sii_unit_id(&graph.vehicle_id)) {
            continue;
        }
        if !is_truck_graph(graph) {
            diagnostics.excluded_trailers += 1;
        } else if looks_like_job_vehicle(graph) {
            diagnostics.excluded_job_vehicles += 1;
        } else {
            diagnostics.excluded_unreferenced += 1;
        }
    }

    diagnostics.owned_trucks = owned.len();
    OwnedTruckCollection {
        owned_ids: owned,
        diagnostics,
    }
}

fn add_owned_id(
    id: &str,
    source: OwnedTruckSource,
    graphs: &HashMap<String, TruckGraph>,
    owned: &mut Vec<String>,
    seen: &mut HashSet<String>,
    diagnostics: &mut OwnedTruckDiagnostics,
) {
    if is_null_ref(id) {
        return;
    }
    let seen_key = normalize_sii_unit_id(id);
    if !seen.insert(seen_key) {
        diagnostics.excluded_duplicates += 1;
        return;
    }
    let Some(graph) = find_truck_graph_by_id(graphs, id) else {
        diagnostics.excluded_invalid += 1;
        return;
    };
    let allow_existing_vehicle_block = matches!(
        source,
        OwnedTruckSource::PlayerTrucksArray | OwnedTruckSource::CurrentTruckPointer
    );
    if !allow_existing_vehicle_block && !is_truck_graph(graph) {
        diagnostics.excluded_trailers += 1;
        return;
    }
    owned.push(graph.vehicle_id.clone());
}

fn is_truck_graph(graph: &TruckGraph) -> bool {
    graph.accessories.iter().any(|accessory| {
        accessory
            .data_path
            .as_deref()
            .map(|path| {
                let normalized = path.replace('\\', "/").to_ascii_lowercase();
                normalized.contains("/def/vehicle/truck/") || normalized.contains("/vehicle/truck/")
            })
            .unwrap_or(false)
    })
}

fn looks_like_job_vehicle(graph: &TruckGraph) -> bool {
    graph.vehicle_id.to_ascii_lowercase().contains("job")
        || graph.vehicle_id.to_ascii_lowercase().contains("quick")
        || graph.accessories.iter().any(|accessory| {
            accessory
                .data_path
                .as_deref()
                .map(|path| {
                    let normalized = path.to_ascii_lowercase();
                    normalized.contains("/company/")
                        || normalized.contains("/quick_job")
                        || normalized.contains("/job_market")
                })
                .unwrap_or(false)
        })
}

pub fn sanitize_sii_display_text(raw: &str) -> String {
    let mut value = decode_common_entities(raw.trim().trim_matches('"'));
    let tag_re = Regex::new(r"<[^>]*>").expect("valid display tag regex");
    value = tag_re.replace_all(&value, " ").to_string();
    let sii_escape_re = Regex::new(r"\\[np]").expect("valid sii escape regex");
    value = sii_escape_re.replace_all(&value, " ").to_string();
    let control_re = Regex::new(r"[\u{0000}-\u{001f}\u{007f}]").expect("valid control regex");
    value = control_re.replace_all(&value, " ").to_string();
    let whitespace_re = Regex::new(r"\s+").expect("valid whitespace regex");
    value = whitespace_re.replace_all(&value, " ").trim().to_string();

    if value.is_empty() {
        return String::new();
    }

    let visible_parts = value
        .split_whitespace()
        .filter(|part| !part.starts_with('@') && !part.starts_with('$'))
        .collect::<Vec<_>>();
    visible_parts.join(" ")
}

fn decode_common_entities(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn garage_display_name(block: &UnitBlock) -> Option<String> {
    extract_first_existing_field(
        &block.raw_block,
        &["display_name", "name", "city", "city_name"],
    )
    .map(|value| sanitize_sii_display_text(&value))
    .filter(|value| is_readable_display_value(value, &block.id))
    .or_else(|| {
        block
            .id
            .split('.')
            .last()
            .map(pretty_token_value)
            .filter(|value| is_readable_display_value(value, &block.id))
    })
}

fn extract_first_existing_field(raw_block: &str, fields: &[&str]) -> Option<String> {
    fields
        .iter()
        .find_map(|field| extract_field_value(raw_block, field))
        .filter(|value| !is_null_ref(value))
}

fn extract_first_existing_field_with_name<'a>(
    raw_block: &str,
    fields: &'a [&str],
) -> Option<(&'a str, String)> {
    fields.iter().find_map(|field| {
        extract_field_value(raw_block, field)
            .filter(|value| !is_null_ref(value))
            .map(|value| (*field, value))
    })
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

fn is_readable_display_value(value: &str, fallback_id: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    let fallback_lower = fallback_id.to_ascii_lowercase();
    !lower.eq(&fallback_lower)
        && !lower.starts_with("driver.")
        && !lower.starts_with("_nameless.")
        && !lower.starts_with("garage.")
}

fn build_inventory(
    content: &str,
    owned_order: &[String],
    active_truck_id: Option<&str>,
    graphs: &HashMap<String, TruckGraph>,
    assignments: &HashMap<String, GarageSlotAssignment>,
    player_vehicle_assignments: &HashMap<String, PlayerVehicleSlotAssignment>,
    driver_infos: &HashMap<String, DriverDisplayInfo>,
) -> Vec<TruckInventoryItem> {
    let parsed_trucks = parse_trucks_from_sii(content);
    let by_id = parsed_trucks
        .iter()
        .map(|truck| (truck.truck_id.clone(), truck))
        .collect::<HashMap<_, _>>();

    owned_order
        .iter()
        .enumerate()
        .filter_map(|(index, truck_id)| {
            let parsed = by_id.get(truck_id)?;
            Some(build_inventory_item(
                index + 1,
                parsed,
                active_truck_id,
                graphs.get(truck_id),
                assignments.get(&normalize_sii_unit_id(truck_id)),
                player_vehicle_assignments.get(&normalize_sii_unit_id(truck_id)),
                driver_infos,
            ))
        })
        .collect()
}

fn build_inventory_item(
    display_index: usize,
    parsed: &ParsedTruck,
    active_truck_id: Option<&str>,
    graph: Option<&TruckGraph>,
    assignment: Option<&GarageSlotAssignment>,
    player_vehicle_assignment: Option<&PlayerVehicleSlotAssignment>,
    driver_infos: &HashMap<String, DriverDisplayInfo>,
) -> TruckInventoryItem {
    let is_active = active_truck_id
        .map(|id| id.eq_ignore_ascii_case(&parsed.truck_id))
        .unwrap_or(false);
    let assigned_driver_id = driver_for_truck(driver_infos, &parsed.truck_id)
        .or_else(|| assignment.and_then(|item| item.driver_id.clone()));
    let is_driver_assigned = assigned_driver_id.is_some() && !is_active;
    let driver_display_name = assigned_driver_id
        .as_deref()
        .and_then(|id| driver_infos.get(&normalize_sii_unit_id(id)))
        .and_then(|info| info.display_name.clone());
    let family = graph.and_then(graph_primary_family);
    let (brand_from_path, model_from_path) = family
        .as_deref()
        .map(brand_model_from_family)
        .unwrap_or((None, None));

    let raw_license_plate = parsed.license_plate.clone();
    let display_license_plate = raw_license_plate
        .as_deref()
        .map(license_plate_display_value)
        .filter(|value| !value.trim().is_empty());
    let country_code_from_plate = raw_license_plate.as_deref().and_then(license_plate_country_code);
    let wear = truck_wear(parsed);

    TruckInventoryItem {
        truck_id: parsed.truck_id.clone(),
        display_index,
        brand: non_empty(&parsed.brand).or(brand_from_path),
        model: non_empty(&parsed.model).or(model_from_path),
        raw_license_plate,
        display_license_plate: display_license_plate.clone(),
        license_plate: display_license_plate,
        garage_id: assignment
            .map(|item| item.garage_id.clone())
            .or_else(|| parsed.assigned_garage.clone()),
        garage_display_name: assignment
            .and_then(|item| item.garage_display_name.clone())
            .or_else(|| parsed.assigned_garage.as_deref().map(pretty_token_value)),
        assigned_garage: assignment
            .map(|item| item.garage_id.clone())
            .or_else(|| parsed.assigned_garage.clone()),
        assigned_driver_id,
        driver_display_name,
        country_code: assignment
            .and_then(|item| item.country_code.clone())
            .or(country_code_from_plate.clone()),
        country_display_name: assignment
            .and_then(|item| item.country_display_name.clone())
            .or_else(|| country_code_from_plate.as_deref().map(pretty_token_value)),
        is_active,
        is_switchable: true,
        blocked_reason: None,
        requires_driver_swap: is_driver_assigned,
        engine_data_path: graph.and_then(graph_engine_data_path),
        transmission_data_path: graph.and_then(graph_transmission_data_path),
        accessory_count: graph.map(|graph| graph.accessory_ids.len()).unwrap_or(0),
        odometer_km: Some(parsed.odometer),
        fuel_relative: Some(parsed.fuel_relative),
        wear,
        player_vehicle_slot_id: player_vehicle_assignment.map(|slot| slot.slot_id.clone()),
        player_vehicle_slot_index: player_vehicle_assignment.and_then(|slot| slot.slot_index),
    }
}

fn driver_for_truck(
    driver_infos: &HashMap<String, DriverDisplayInfo>,
    truck_id: &str,
) -> Option<String> {
    let normalized = normalize_sii_unit_id(truck_id);
    let mut drivers = driver_infos
        .values()
        .filter(|driver| driver.unit_type != "driver_player")
        .filter(|driver| {
            driver
                .current_truck_id_normalized
                .as_deref()
                .map(|candidate| candidate == normalized)
                .unwrap_or(false)
        })
        .map(|driver| driver.driver_id.clone())
        .collect::<Vec<_>>();
    drivers.sort();
    drivers.dedup();
    if drivers.len() == 1 {
        drivers.pop()
    } else {
        None
    }
}

fn truck_wear(parsed: &ParsedTruck) -> Option<f32> {
    let mut values = vec![
        parsed.engine_wear,
        parsed.transmission_wear,
        parsed.cabin_wear,
        parsed.chassis_wear,
    ];
    values.extend(parsed.wheels_wear.iter().copied());
    values.retain(|value| value.is_finite());
    if values.is_empty() {
        None
    } else {
        Some(values.into_iter().fold(0.0_f32, f32::max))
    }
}

fn license_plate_display_value(raw: &str) -> String {
    let visible = raw.split('|').next().unwrap_or(raw);
    sanitize_sii_display_text(visible)
}

fn license_plate_country_code(raw: &str) -> Option<String> {
    raw.split_once('|')
        .map(|(_, country)| country.trim().to_string())
        .filter(|country| !country.is_empty())
}

fn build_truck_assignments(
    trucks: &[TruckInventoryItem],
    assignments: &HashMap<String, GarageSlotAssignment>,
    driver_infos: &HashMap<String, DriverDisplayInfo>,
    active_truck_id: Option<&str>,
) -> HashMap<String, TruckAssignment> {
    trucks
        .iter()
        .map(|truck| {
            let assignment = assignments.get(&normalize_sii_unit_id(&truck.truck_id));
            let driver_id = assignment.and_then(|item| item.driver_id.clone());
            let driver_name = driver_id
                .as_deref()
                .and_then(|id| driver_infos.get(&normalize_sii_unit_id(id)))
                .and_then(|info| info.display_name.clone());
            (
                truck.truck_id.clone(),
                TruckAssignment {
                    truck_id: truck.truck_id.clone(),
                    driver_id,
                    driver_name,
                    garage_id: assignment.map(|item| item.garage_id.clone()),
                    garage_name: assignment.and_then(|item| item.garage_display_name.clone()),
                    is_player_truck: active_truck_id
                        .map(|id| id.eq_ignore_ascii_case(&truck.truck_id))
                        .unwrap_or(false),
                },
            )
        })
        .collect()
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_array_entries, extract_array_values, graph_dangling_accessories,
        is_valid_garage_driver_ref, normalize_sii_unit_id, parse_truck_save,
        resolve_current_truck_pointer,
    };
    use crate::features::truck_change::models::CurrentTruckPointerKind;

    fn fixture() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 assigned_truck: _nameless.truck.active
 my_truck: _nameless.truck.active
 trucks: 3
 trucks[0]: _nameless.truck.active
 trucks[1]: _nameless.truck.free
 trucks[2]: _nameless.truck.driver
 drivers: 1
 drivers[0]: driver.1
}
vehicle : _nameless.truck.active {
 accessories: 1
 accessories[0]: _nameless.acc.active
}
vehicle_accessory : _nameless.acc.active {
 data_path: "/def/vehicle/truck/scania.s_2016/engine/dc16_770.sii"
}
vehicle : _nameless.truck.free {
 accessories: 2
 accessories[0]: _nameless.acc.free
 accessories[1]: _nameless.acc.free_tr
}
vehicle_accessory : _nameless.acc.free {
 data_path: "/def/vehicle/truck/scania.s_2016/data.sii"
}
vehicle_accessory : _nameless.acc.free_tr {
 data_path: "/def/vehicle/truck/scania.s_2016/transmission/g38cm_r.sii"
}
vehicle : _nameless.truck.driver {
 accessories: 1
 accessories[0]: _nameless.acc.driver
}
vehicle_accessory : _nameless.acc.driver {
 data_path: "/def/vehicle/truck/man.tgx/data.sii"
}
garage : garage.berlin {
 vehicles: 3
 vehicles[0]: _nameless.truck.active
 vehicles[1]: _nameless.truck.free
 vehicles[2]: _nameless.truck.driver
 drivers: 3
 drivers[0]: null
 drivers[1]: null
 drivers[2]: driver.2
}
driver : driver.2 {
 first_name: "Max"
 last_name: "Mustermann"
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
 my_truck: null
 assigned_truck: null
}
player_vehicles : _nameless.assigned.1 {
 vehicle: _nameless.truck.4
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

    #[test]
    fn inventory_keeps_save_order_and_separate_ids() {
        let parsed = parse_truck_save(fixture());
        assert_eq!(parsed.trucks.len(), 3);
        assert_eq!(parsed.trucks[0].truck_id, "_nameless.truck.active");
        assert_eq!(parsed.trucks[1].truck_id, "_nameless.truck.free");
        assert_eq!(parsed.trucks[2].truck_id, "_nameless.truck.driver");
        assert!(parsed.trucks[0].is_active);
    }

    #[test]
    fn assigned_vehicles_pointer_resolves_current_truck_with_null_my_truck() {
        let parsed = parse_truck_save(assigned_vehicles_fixture());
        let pointer = resolve_current_truck_pointer(&parsed).unwrap();

        assert_eq!(
            pointer.kind,
            CurrentTruckPointerKind::PlayerAssignedVehicles
        );
        assert_eq!(pointer.truck_id, "_nameless.truck.4");
        assert_eq!(pointer.owner_unit_id, "_nameless.assigned.1");
        assert_eq!(pointer.field_name, "vehicle");
        assert_eq!(
            pointer.referenced_player_vehicle_unit_id.as_deref(),
            Some("_nameless.assigned.1")
        );
        assert_eq!(parsed.active_truck_id.as_deref(), Some("_nameless.truck.4"));
        assert_eq!(
            parsed.current_truck_diagnostics.my_truck_raw.as_deref(),
            Some("null")
        );
        assert_eq!(
            parsed.diagnostics.current_truck_pointer_kind,
            Some(CurrentTruckPointerKind::PlayerAssignedVehicles)
        );
        assert_eq!(
            parsed.diagnostics.assigned_vehicles_unit_id.as_deref(),
            Some("_nameless.assigned.1")
        );
    }

    #[test]
    fn player_trucks_array_is_primary_ownership_source() {
        let parsed = parse_truck_save(assigned_vehicles_fixture());

        assert_eq!(parsed.trucks.len(), 5);
        assert_eq!(parsed.diagnostics.player_trucks_array_count, 5);
        assert_eq!(parsed.diagnostics.player_truck_refs_with_vehicle_blocks, 5);
        assert_eq!(parsed.diagnostics.owned_trucks, 5);
        assert!(parsed
            .trucks
            .iter()
            .any(|truck| { truck.truck_id == "_nameless.truck.4" && truck.is_active }));
    }

    #[test]
    fn missing_assigned_vehicles_unit_is_diagnosed() {
        let content = assigned_vehicles_fixture().replace(
            "player_vehicles : _nameless.assigned.1",
            "player_vehicles : _nameless.assigned.other",
        );
        let parsed = parse_truck_save(&content);
        let pointer = resolve_current_truck_pointer(&parsed).unwrap();

        assert_eq!(pointer.kind, CurrentTruckPointerKind::FallbackPlayerVehicles);
        assert!(parsed.current_truck_diagnostics.player_found);
        assert_eq!(
            parsed
                .current_truck_diagnostics
                .assigned_vehicles_raw
                .as_deref(),
            Some("_nameless.assigned.1")
        );
        assert!(
            !parsed
                .current_truck_diagnostics
                .assigned_vehicles_unit_found
        );
    }

    #[test]
    fn missing_assigned_vehicles_vehicle_block_is_diagnosed() {
        let content = assigned_vehicles_fixture().replace(
            "vehicle : _nameless.truck.4",
            "vehicle : _nameless.truck.other",
        );
        let parsed = parse_truck_save(&content);
        let pointer = resolve_current_truck_pointer(&parsed).unwrap();

        assert_eq!(pointer.kind, CurrentTruckPointerKind::FallbackFirstOwnedTruck);
        assert!(
            parsed
                .current_truck_diagnostics
                .assigned_vehicles_unit_found
        );
        assert_eq!(
            parsed
                .current_truck_diagnostics
                .assigned_vehicles_vehicle_raw
                .as_deref(),
            Some("_nameless.truck.4")
        );
        assert!(
            !parsed
                .current_truck_diagnostics
                .assigned_vehicles_vehicle_block_found
        );
    }

    #[test]
    fn player_trucks_array_missing_vehicle_blocks_are_diagnosed_and_hidden() {
        let content = assigned_vehicles_fixture().replace(
            "trucks[1]: _nameless.truck.2",
            "trucks[1]: _nameless.truck.missing",
        );
        let parsed = parse_truck_save(&content);

        assert_eq!(parsed.diagnostics.player_trucks_array_count, 5);
        assert_eq!(parsed.diagnostics.player_truck_refs_with_vehicle_blocks, 4);
        assert_eq!(
            parsed
                .diagnostics
                .player_truck_reference_missing_vehicle_blocks,
            vec!["_nameless.truck.missing".to_string()]
        );
        assert_eq!(parsed.trucks.len(), 4);
        assert!(!parsed
            .trucks
            .iter()
            .any(|truck| truck.truck_id == "_nameless.truck.missing"));
    }

    #[test]
    fn driver_assigned_truck_is_visible_and_switchable_with_swap() {
        let parsed = parse_truck_save(fixture());
        let driver_truck = parsed
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.driver")
            .unwrap();
        assert!(driver_truck.is_switchable);
        assert!(driver_truck.requires_driver_swap);
        assert_eq!(
            driver_truck.driver_display_name.as_deref(),
            Some("Max Mustermann")
        );
    }

    #[test]
    fn graph_detects_engine_and_transmission_accessories() {
        let parsed = parse_truck_save(fixture());
        let free = parsed
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.free")
            .unwrap();
        assert_eq!(
            free.transmission_data_path.as_deref(),
            Some("/def/vehicle/truck/scania.s_2016/transmission/g38cm_r.sii")
        );
    }

    #[test]
    fn dangling_accessory_reference_is_reported() {
        let content = fixture().replace(
            "accessories[1]: _nameless.acc.free_tr",
            "accessories[1]: _nameless.acc.missing",
        );
        let parsed = parse_truck_save(&content);
        let graph = parsed.truck_graphs.get("_nameless.truck.free").unwrap();
        assert_eq!(
            graph_dangling_accessories(graph, &parsed.unit_ids),
            vec!["_nameless.acc.missing".to_string()]
        );
    }

    #[test]
    fn sanitize_sii_display_text_removes_formatting_tags_without_mutating_raw_value() {
        let raw = "HH<offset hshift=12>AB 123";
        assert_eq!(super::sanitize_sii_display_text(raw), "HH AB 123");
        assert_eq!(raw, "HH<offset hshift=12>AB 123");
    }

    #[test]
    fn array_values_keep_numeric_slot_order_with_duplicate_values() {
        let raw = r#"garage : garage.berlin {
 drivers: 3
 drivers[2]: driver.1
 drivers[0]: null
 drivers[1]: driver.1
}"#;
        assert_eq!(
            extract_array_values(raw, "drivers"),
            vec![
                "null".to_string(),
                "driver.1".to_string(),
                "driver.1".to_string()
            ]
        );
    }

    #[test]
    fn normalize_sii_unit_id_handles_safe_lookup_variants() {
        assert_eq!(normalize_sii_unit_id(" driver.1 "), "driver.1");
        assert_eq!(normalize_sii_unit_id("\"Driver.1\""), "driver.1");
        assert_eq!(
            normalize_sii_unit_id(" _nameless.Truck.ABC, "),
            "_nameless.truck.abc"
        );
        assert_ne!(
            normalize_sii_unit_id("driver.abc"),
            normalize_sii_unit_id("driver.abd")
        );
    }

    #[test]
    fn garage_driver_ref_validation_accepts_only_exact_driver_numbers() {
        assert!(is_valid_garage_driver_ref("driver.163"));
        assert!(is_valid_garage_driver_ref("driver.0"));
        assert!(is_valid_garage_driver_ref("driver.999"));
        assert!(!is_valid_garage_driver_ref("null"));
        assert!(!is_valid_garage_driver_ref("nil"));
        assert!(!is_valid_garage_driver_ref("none"));
        assert!(!is_valid_garage_driver_ref("0"));
        assert!(!is_valid_garage_driver_ref(""));
        assert!(!is_valid_garage_driver_ref("_nameless.truck.123"));
        assert!(!is_valid_garage_driver_ref("driver.abc"));
    }

    #[test]
    fn array_entries_preserve_sparse_indices() {
        let raw = r#"garage : garage.berlin {
 vehicles: 4
 vehicles[2]: _nameless.truck.two
 vehicles[3]: _nameless.truck.three
 drivers: 4
 drivers[3]: driver.3
}"#;
        assert_eq!(
            extract_array_entries(raw, "vehicles"),
            vec![
                (2, "_nameless.truck.two".to_string()),
                (3, "_nameless.truck.three".to_string())
            ]
        );
        assert_eq!(
            extract_array_entries(raw, "drivers"),
            vec![(3, "driver.3".to_string())]
        );
    }

    #[test]
    fn garage_slots_pair_by_explicit_index_without_shifting() {
        let content = fixture().replace(" drivers[1]: null\n", "");
        let parsed = parse_truck_save(&content);
        let free = parsed
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.free")
            .unwrap();
        let driver = parsed
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.driver")
            .unwrap();

        assert_eq!(free.assigned_driver_id, None);
        assert_eq!(driver.assigned_driver_id.as_deref(), Some("driver.2"));
        assert!(
            !parsed
                .garage_assignments
                .get(&normalize_sii_unit_id("_nameless.truck.driver"))
                .unwrap()
                .arrays_have_matching_indices
        );
    }

    #[test]
    fn driver_parser_reports_recognized_and_ignored_driver_like_blocks() {
        let content = fixture().replace(
            "driver : driver.2 {",
            "employee : employee.driver_like {\n assigned_truck: _nameless.truck.driver\n}\ndriver : driver.2 {",
        );
        let parsed = parse_truck_save(&content);

        assert_eq!(parsed.driver_diagnostics.recognized_driver_blocks, 1);
        assert_eq!(
            parsed.driver_diagnostics.recognized_unit_types,
            vec!["driver".to_string()]
        );
        assert_eq!(parsed.driver_diagnostics.ignored_driver_like_blocks, 1);
    }

    #[test]
    fn driver_ai_and_driver_player_blocks_are_recognized() {
        let content = fixture().replace(
            "driver : driver.2 {",
            "driver_player : driver.1 {\n profit_log: null\n}\ndriver_ai : driver.2 {",
        );
        let parsed = parse_truck_save(&content);

        assert_eq!(parsed.driver_diagnostics.recognized_driver_blocks, 2);
        assert_eq!(
            parsed.driver_diagnostics.recognized_unit_types,
            vec!["driver_ai".to_string(), "driver_player".to_string()]
        );
        assert!(parsed.driver_infos.contains_key("driver.1"));
        assert!(parsed.driver_infos.contains_key("driver.2"));
    }
}
