use std::collections::{BTreeSet, HashMap, HashSet};

use regex::Regex;

use crate::models::trucks::ParsedTruck;
use crate::shared::sii_parser::{get_player_id, get_vehicle_ids, parse_trucks_from_sii};

use super::models::{GarageCapacity, GarageSlotAssignment, TruckGraph, TruckInventoryItem};

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
    let trucks = build_inventory(
        content,
        &truck_order,
        active_truck_id.as_deref(),
        &truck_graphs,
        &garage_scan.assignments,
    );

    ParsedTruckSave {
        active_truck_id,
        player_id,
        truck_order,
        trucks,
        truck_graphs,
        garage_assignments: garage_scan.assignments,
        garages: garage_scan.garages,
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
    let prefix = format!("{}[", field);
    let mut values = raw_block
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with(&prefix) {
                return None;
            }
            trimmed
                .split_once(':')
                .map(|(_, value)| normalize_value(value.trim()))
                .filter(|value| !value.is_empty())
        })
        .collect::<Vec<_>>();
    values.sort_by_key(|value| {
        raw_block
            .lines()
            .position(|line| line.contains(value))
            .unwrap_or(usize::MAX)
    });
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

fn parse_garages(unit_blocks: &[UnitBlock]) -> GarageScan {
    let mut assignments = HashMap::new();
    let mut garages = Vec::new();

    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "garage")
    {
        let vehicles = extract_array_values(&block.raw_block, "vehicles");
        let drivers = extract_array_values(&block.raw_block, "drivers");
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

fn build_inventory(
    content: &str,
    truck_order: &[String],
    active_truck_id: Option<&str>,
    graphs: &HashMap<String, TruckGraph>,
    assignments: &HashMap<String, GarageSlotAssignment>,
) -> Vec<TruckInventoryItem> {
    let parsed_trucks = parse_trucks_from_sii(content);
    let by_id = parsed_trucks
        .iter()
        .map(|truck| (truck.truck_id.clone(), truck))
        .collect::<HashMap<_, _>>();
    let mut ordered_ids = truck_order.to_vec();
    for truck in &parsed_trucks {
        if !ordered_ids.iter().any(|id| id == &truck.truck_id) {
            ordered_ids.push(truck.truck_id.clone());
        }
    }

    ordered_ids
        .into_iter()
        .enumerate()
        .filter_map(|(index, truck_id)| {
            let parsed = by_id.get(&truck_id)?;
            Some(build_inventory_item(
                index + 1,
                parsed,
                active_truck_id,
                graphs.get(&truck_id),
                assignments.get(&truck_id),
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
) -> TruckInventoryItem {
    let is_active = active_truck_id
        .map(|id| id.eq_ignore_ascii_case(&parsed.truck_id))
        .unwrap_or(false);
    let assigned_driver_id = assignment.and_then(|item| item.driver_id.clone());
    let is_driver_assigned = assigned_driver_id.is_some() && !is_active;
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
        license_plate: parsed.license_plate.clone(),
        assigned_garage: assignment
            .map(|item| item.garage_id.clone())
            .or_else(|| parsed.assigned_garage.clone()),
        assigned_driver_id,
        is_active,
        is_switchable: !is_driver_assigned,
        blocked_reason: if is_driver_assigned {
            Some("truck_assigned_to_driver".to_string())
        } else {
            None
        },
        engine_data_path: graph.and_then(graph_engine_data_path),
        transmission_data_path: graph.and_then(graph_transmission_data_path),
        accessory_count: graph.map(|graph| graph.accessory_ids.len()).unwrap_or(0),
    }
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
    use super::{graph_dangling_accessories, parse_truck_save};

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
    fn driver_assigned_truck_is_visible_but_not_switchable() {
        let parsed = parse_truck_save(fixture());
        let driver_truck = parsed
            .trucks
            .iter()
            .find(|truck| truck.truck_id == "_nameless.truck.driver")
            .unwrap();
        assert!(!driver_truck.is_switchable);
        assert_eq!(
            driver_truck.blocked_reason.as_deref(),
            Some("truck_assigned_to_driver")
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
}
