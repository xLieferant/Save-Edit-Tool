use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use super::models::{GarageCapacity, TruckGraph, TruckTransferPreview, TruckTransferSelection};
use super::parser::{ParsedTruckSave, UnitBlock, parse_truck_save};

const MAX_TRANSFER_GRAPH_DEPTH: usize = 8;

const ALLOWED_TRANSFER_UNIT_TYPES: &[&str] = &[
    "vehicle",
    "vehicle_accessory",
    "vehicle_addon_accessory",
    "accessory_hookup_data",
];

pub fn preview_truck_transfer_from_content(
    source_content: &str,
    target_content: &str,
    selections: &[TruckTransferSelection],
) -> TruckTransferPreview {
    let source = parse_truck_save(source_content);
    let target = parse_truck_save(target_content);
    let mut warnings = Vec::new();
    let mut source_graphs = Vec::new();
    let mut source_ids = BTreeMap::new();

    for selection in selections {
        match collect_limited_graph(&source, &selection.truck_id, MAX_TRANSFER_GRAPH_DEPTH) {
            Ok(graph) => {
                source_ids.insert(graph.vehicle_id.clone(), ());
                for id in &graph.accessory_ids {
                    source_ids.insert(id.clone(), ());
                }
                for id in &graph.referenced_unit_ids {
                    if source.unit_ids.contains(id) {
                        source_ids.insert(id.clone(), ());
                    } else {
                        warnings.push(format!("unknown_external_reference:{}", id));
                    }
                }
                source_graphs.push(graph);
            }
            Err(error) => warnings.push(error),
        }
    }

    let selected_truck_count = selections.len();
    let free_truck_slots = target
        .garages
        .iter()
        .map(|garage| garage.free_truck_slots)
        .sum::<usize>();
    let can_apply_capacity = selected_truck_count <= free_truck_slots;
    let error = if can_apply_capacity {
        None
    } else {
        Some("insufficient_garage_capacity".to_string())
    };
    let source_ids = source_ids.into_keys().collect::<Vec<_>>();
    let id_remap = generate_id_remap(&source_ids, &target.unit_ids);

    TruckTransferPreview {
        selected_truck_count,
        free_truck_slots,
        can_apply: can_apply_capacity && warnings.is_empty(),
        error,
        source_graphs,
        id_remap,
        target_garages: target.garages.clone(),
        warnings,
    }
}

pub fn generate_id_remap(
    source_ids: &[String],
    target_existing_ids: &HashSet<String>,
) -> HashMap<String, String> {
    let mut remap = HashMap::new();
    let mut used = target_existing_ids.clone();
    for (index, source_id) in source_ids.iter().enumerate() {
        let mut suffix = index + 1;
        loop {
            let candidate = format!("_nameless.truck_change.{:08x}", suffix);
            if !used.contains(&candidate) {
                used.insert(candidate.clone());
                remap.insert(source_id.clone(), candidate);
                break;
            }
            suffix += source_ids.len().max(1);
        }
    }
    remap
}

pub fn rewrite_graph_block_references(raw_block: &str, remap: &HashMap<String, String>) -> String {
    let mut rewritten = raw_block.to_string();
    let mut keys = remap.keys().cloned().collect::<Vec<_>>();
    keys.sort_by_key(|key| std::cmp::Reverse(key.len()));
    for key in keys {
        if let Some(value) = remap.get(&key) {
            rewritten = rewritten.replace(&key, value);
        }
    }
    rewritten
}

fn collect_limited_graph(
    parsed: &ParsedTruckSave,
    vehicle_id: &str,
    max_depth: usize,
) -> Result<TruckGraph, String> {
    let base = parsed
        .truck_graphs
        .get(vehicle_id)
        .cloned()
        .ok_or_else(|| format!("source_truck_not_found:{}", vehicle_id))?;
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut referenced = BTreeMap::new();

    queue.push_back((vehicle_id.to_string(), 0usize));
    for id in &base.accessory_ids {
        queue.push_back((id.clone(), 1usize));
    }

    while let Some((id, depth)) = queue.pop_front() {
        if depth > max_depth {
            return Err(format!("transfer_graph_depth_exceeded:{}", vehicle_id));
        }
        if !visited.insert(id.clone()) {
            continue;
        }
        let Some(block) = parsed.unit_blocks.get(&id) else {
            referenced.insert(id, ());
            continue;
        };
        if !is_allowed_transfer_unit(block) {
            referenced.insert(id, ());
            continue;
        }
        for reference in extract_block_references(block) {
            if reference == id {
                continue;
            }
            referenced.insert(reference.clone(), ());
            if parsed.unit_ids.contains(&reference) && !visited.contains(&reference) {
                queue.push_back((reference, depth + 1));
            }
        }
    }

    let mut graph = base;
    graph.referenced_unit_ids = referenced.into_keys().collect();
    Ok(graph)
}

fn is_allowed_transfer_unit(block: &UnitBlock) -> bool {
    ALLOWED_TRANSFER_UNIT_TYPES
        .iter()
        .any(|unit_type| *unit_type == block.unit_type)
}

fn extract_block_references(block: &UnitBlock) -> Vec<String> {
    let mut references = Vec::new();
    for token in block.raw_block.split_whitespace() {
        let normalized = token
            .trim_matches(',')
            .trim_matches('"')
            .trim_end_matches('}')
            .to_string();
        if normalized.starts_with("_nameless.") {
            references.push(normalized);
        }
    }
    references.sort();
    references.dedup();
    references
}

pub fn target_free_slots(garages: &[GarageCapacity]) -> usize {
    garages.iter().map(|garage| garage.free_truck_slots).sum()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        generate_id_remap, preview_truck_transfer_from_content, rewrite_graph_block_references,
    };
    use crate::features::truck_change::models::TruckTransferSelection;

    #[test]
    fn id_remapping_avoids_target_collisions() {
        let source_ids = vec![
            "_nameless.source.a".to_string(),
            "_nameless.source.b".to_string(),
        ];
        let mut target = HashSet::new();
        target.insert("_nameless.truck_change.00000001".to_string());

        let remap = generate_id_remap(&source_ids, &target);
        let values = remap.values().cloned().collect::<HashSet<_>>();
        assert_eq!(values.len(), source_ids.len());
        assert!(!values.contains("_nameless.truck_change.00000001"));
    }

    #[test]
    fn remapped_graph_block_contains_no_old_source_ids() {
        let mut remap = std::collections::HashMap::new();
        remap.insert(
            "_nameless.source.a".to_string(),
            "_nameless.truck_change.00000001".to_string(),
        );
        remap.insert(
            "_nameless.source.acc".to_string(),
            "_nameless.truck_change.00000002".to_string(),
        );
        let rewritten = rewrite_graph_block_references(
            "vehicle : _nameless.source.a { accessories[0]: _nameless.source.acc }",
            &remap,
        );
        assert!(!rewritten.contains("_nameless.source.a"));
        assert!(!rewritten.contains("_nameless.source.acc"));
    }

    #[test]
    fn transfer_preview_blocks_when_garage_capacity_is_insufficient() {
        let source = r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 my_truck: _nameless.truck.a
 trucks: 1
 trucks[0]: _nameless.truck.a
}
vehicle : _nameless.truck.a {
 accessories: 1
 accessories[0]: _nameless.acc.a
}
vehicle_accessory : _nameless.acc.a {
 data_path: "/def/vehicle/truck/scania.s_2016/data.sii"
}
}
"#;
        let target = r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 my_truck: _nameless.truck.target
 trucks: 1
 trucks[0]: _nameless.truck.target
}
vehicle : _nameless.truck.target {
 accessories: 1
 accessories[0]: _nameless.acc.target
}
vehicle_accessory : _nameless.acc.target {
 data_path: "/def/vehicle/truck/man.tgx/data.sii"
}
garage : garage.berlin {
 vehicles: 1
 vehicles[0]: _nameless.truck.target
 drivers: 1
 drivers[0]: null
}
}
"#;
        let preview = preview_truck_transfer_from_content(
            source,
            target,
            &[TruckTransferSelection {
                truck_id: "_nameless.truck.a".to_string(),
                target_garage_id: Some("garage.berlin".to_string()),
            }],
        );
        assert!(!preview.can_apply);
        assert_eq!(
            preview.error.as_deref(),
            Some("insufficient_garage_capacity")
        );
    }
}
