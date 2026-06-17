use std::collections::{BTreeSet, HashMap, HashSet};

use regex::Regex;

use crate::models::trucks::ParsedTruck;
use crate::shared::sii_parser::{get_player_id, get_vehicle_ids, parse_trucks_from_sii};

use super::models::{
    DriverDisplayInfo, GarageCapacity, GarageSlotAssignment, OwnedTruckDiagnostics,
    TruckAssignment, TruckGraph, TruckInventoryItem,
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
    pub player_id: Option<String>,
    pub truck_order: Vec<String>,
    pub trucks: Vec<TruckInventoryItem>,
    pub truck_graphs: HashMap<String, TruckGraph>,
    pub garage_assignments: HashMap<String, GarageSlotAssignment>,
    pub garages: Vec<GarageCapacity>,
    pub driver_infos: HashMap<String, DriverDisplayInfo>,
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
    let active_truck_id = player_id
        .as_ref()
        .and_then(|id| get_vehicle_ids(content, id).0)
        .filter(|value| !is_null_ref(value));
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
    let garage_scan = parse_garages(&unit_blocks);
    let driver_infos = parse_driver_infos(&unit_blocks);
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
    let owned_collection = collect_owned_player_truck_ids(
        &unit_blocks,
        &truck_order,
        active_truck_id.as_deref(),
        &truck_graphs,
        &garage_scan.assignments,
    );
    crate::dev_log!(
        "[truck_change] owned truck collection completed vehicle_blocks={} candidate_trucks={} owned_trucks={}",
        owned_collection.diagnostics.total_vehicle_blocks,
        owned_collection.diagnostics.candidate_trucks,
        owned_collection.diagnostics.owned_trucks
    );
    crate::dev_log!(
        "[truck_change] excluded non-owned vehicle blocks: {}",
        owned_collection.diagnostics.excluded_unreferenced
            + owned_collection.diagnostics.excluded_job_vehicles
            + owned_collection.diagnostics.excluded_invalid
    );
    let trucks = build_inventory(
        content,
        &owned_collection.owned_ids,
        active_truck_id.as_deref(),
        &truck_graphs,
        &garage_scan.assignments,
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
        player_id,
        truck_order,
        trucks,
        truck_graphs,
        garage_assignments: garage_scan.assignments,
        garages: garage_scan.garages,
        driver_infos,
        truck_assignments,
        diagnostics: owned_collection.diagnostics,
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
    values.into_iter().map(|(_, value)| value).collect()
}

pub fn normalize_value(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(',')
        .trim_matches('"')
        .trim()
        .to_string()
}

pub fn is_null_ref(value: &str) -> bool {
    let normalized = value.trim();
    normalized.is_empty()
        || normalized.eq_ignore_ascii_case("null")
        || normalized.eq_ignore_ascii_case("nil")
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
        .find_map(truck_family_from_data_path)
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

struct OwnedTruckCollection {
    owned_ids: Vec<String>,
    diagnostics: OwnedTruckDiagnostics,
}

fn parse_garages(unit_blocks: &[UnitBlock]) -> GarageScan {
    let mut assignments = HashMap::new();
    let mut garages = Vec::new();

    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "garage")
    {
        let vehicles = extract_array_values(&block.raw_block, "vehicles");
        let drivers = extract_array_values(&block.raw_block, "drivers");
        let garage_display_name = garage_display_name(block);
        let country_code = extract_first_existing_field(
            &block.raw_block,
            &["country", "country_code", "country_token"],
        );
        let country_display_name = country_code.as_deref().map(pretty_token_value);
        let mut occupied = 0usize;
        let mut free = 0usize;

        for (index, truck_id) in vehicles.iter().enumerate() {
            if is_null_ref(truck_id) {
                free += 1;
                continue;
            }
            occupied += 1;
            let driver_id = drivers
                .get(index)
                .filter(|value| !is_null_ref(value))
                .cloned();
            assignments.insert(
                truck_id.clone(),
                GarageSlotAssignment {
                    garage_id: block.id.clone(),
                    garage_display_name: garage_display_name.clone(),
                    country_code: country_code.clone(),
                    country_display_name: country_display_name.clone(),
                    slot_index: index,
                    truck_id: truck_id.clone(),
                    driver_id,
                },
            );
        }

        if vehicles.is_empty() {
            let total = extract_field_value(&block.raw_block, "vehicles")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            free = total;
        }

        garages.push(GarageCapacity {
            garage_id: block.id.clone(),
            garage_display_name: garage_display_name.clone(),
            total_truck_slots: occupied + free,
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
        let vehicles = extract_array_values(&block.raw_block, "vehicles");
        let drivers = extract_array_values(&block.raw_block, "drivers");

        for (index, truck_id) in vehicles.iter().enumerate() {
            if is_null_ref(truck_id) {
                continue;
            }

            let truck_slot = format!("{}:{}", block.id, index);
            if let Some(previous_slot) = truck_slots.insert(truck_id.clone(), truck_slot.clone()) {
                conflicts.push(format!(
                    "duplicate_truck_assignment:{}:{}:{}",
                    truck_id, previous_slot, truck_slot
                ));
            }

            let Some(driver_id) = drivers.get(index).filter(|value| !is_null_ref(value)) else {
                continue;
            };

            if let Some(previous_slot) = driver_slots.insert(driver_id.clone(), truck_slot.clone())
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

fn parse_driver_infos(unit_blocks: &[UnitBlock]) -> HashMap<String, DriverDisplayInfo> {
    unit_blocks
        .iter()
        .filter(|block| block.unit_type == "driver")
        .map(|block| {
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
            let current_truck_id =
                extract_first_existing_field(&block.raw_block, &["assigned_truck", "truck"]);

            (
                block.id.clone(),
                DriverDisplayInfo {
                    driver_id: block.id.clone(),
                    display_name,
                    current_truck_id,
                },
            )
        })
        .collect()
}

pub fn collect_owned_player_truck_ids_from_save(parsed: &ParsedTruckSave) -> OwnedTruckDiagnostics {
    parsed.diagnostics.clone()
}

fn collect_owned_player_truck_ids(
    unit_blocks: &[UnitBlock],
    truck_order: &[String],
    active_truck_id: Option<&str>,
    graphs: &HashMap<String, TruckGraph>,
    assignments: &HashMap<String, GarageSlotAssignment>,
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

    for id in truck_order {
        add_owned_id(id, graphs, &mut owned, &mut seen, &mut diagnostics);
    }
    if let Some(active) = active_truck_id {
        add_owned_id(active, graphs, &mut owned, &mut seen, &mut diagnostics);
    }

    let mut garage_ids = assignments.keys().cloned().collect::<Vec<_>>();
    garage_ids.sort_by_key(|id| {
        assignments
            .get(id)
            .map(|item| (item.garage_id.clone(), item.slot_index))
            .unwrap_or_else(|| (String::new(), usize::MAX))
    });
    for id in garage_ids {
        add_owned_id(&id, graphs, &mut owned, &mut seen, &mut diagnostics);
    }

    for graph in graphs.values() {
        if seen.contains(&graph.vehicle_id) {
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
    graphs: &HashMap<String, TruckGraph>,
    owned: &mut Vec<String>,
    seen: &mut HashSet<String>,
    diagnostics: &mut OwnedTruckDiagnostics,
) {
    if is_null_ref(id) {
        return;
    }
    if !seen.insert(id.to_string()) {
        diagnostics.excluded_duplicates += 1;
        return;
    }
    let Some(graph) = graphs.get(id) else {
        diagnostics.excluded_invalid += 1;
        return;
    };
    if !is_truck_graph(graph) {
        diagnostics.excluded_trailers += 1;
        return;
    }
    owned.push(id.to_string());
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
                assignments.get(truck_id),
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
    driver_infos: &HashMap<String, DriverDisplayInfo>,
) -> TruckInventoryItem {
    let is_active = active_truck_id
        .map(|id| id.eq_ignore_ascii_case(&parsed.truck_id))
        .unwrap_or(false);
    let assigned_driver_id = assignment.and_then(|item| item.driver_id.clone());
    let is_driver_assigned = assigned_driver_id.is_some() && !is_active;
    let driver_display_name = assigned_driver_id
        .as_deref()
        .and_then(|id| driver_infos.get(id))
        .and_then(|info| info.display_name.clone());
    let family = graph.and_then(graph_primary_family);
    let (brand_from_path, model_from_path) = family
        .as_deref()
        .map(brand_model_from_family)
        .unwrap_or((None, None));

    TruckInventoryItem {
        truck_id: parsed.truck_id.clone(),
        display_index,
        brand: non_empty(&parsed.brand).or(brand_from_path),
        model: non_empty(&parsed.model).or(model_from_path),
        raw_license_plate: parsed.license_plate.clone(),
        display_license_plate: parsed
            .license_plate
            .as_deref()
            .map(sanitize_sii_display_text)
            .filter(|value| !value.trim().is_empty()),
        license_plate: parsed
            .license_plate
            .as_deref()
            .map(sanitize_sii_display_text)
            .filter(|value| !value.trim().is_empty()),
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
        country_code: assignment.and_then(|item| item.country_code.clone()),
        country_display_name: assignment.and_then(|item| item.country_display_name.clone()),
        is_active,
        is_switchable: true,
        blocked_reason: None,
        requires_driver_swap: is_driver_assigned,
        engine_data_path: graph.and_then(graph_engine_data_path),
        transmission_data_path: graph.and_then(graph_transmission_data_path),
        accessory_count: graph.map(|graph| graph.accessory_ids.len()).unwrap_or(0),
    }
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
            let assignment = assignments.get(&truck.truck_id);
            let driver_id = assignment.and_then(|item| item.driver_id.clone());
            let driver_name = driver_id
                .as_deref()
                .and_then(|id| driver_infos.get(id))
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
    use super::{extract_array_values, graph_dangling_accessories, parse_truck_save};

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
}
