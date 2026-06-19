use super::models::TruckWriteValidation;
use super::parser::{
    assignment_conflicts_from_blocks, garage_driver_ref_is_unique, graph_dangling_accessories,
    is_valid_garage_driver_ref, normalize_sii_unit_id, parse_truck_save,
    resolve_current_truck_pointer,
};

pub fn validate_truck_switch_content(
    content: &str,
    expected_truck_id: &str,
    expected_driver_id: Option<&str>,
    expected_driver_truck_id: Option<&str>,
) -> TruckWriteValidation {
    let parsed = parse_truck_save(content);
    let actual_truck_id = resolve_current_truck_pointer(&parsed)
        .ok()
        .map(|pointer| pointer.truck_id);
    let mut dangling_references = Vec::new();
    let mut errors = Vec::new();

    match actual_truck_id.as_deref() {
        Some(actual) if actual.eq_ignore_ascii_case(expected_truck_id) => {}
        Some(_) => errors.push("active_truck_mismatch".to_string()),
        None => errors.push("current_truck_not_found".to_string()),
    }

    if !parsed
        .trucks
        .iter()
        .any(|truck| truck.truck_id.eq_ignore_ascii_case(expected_truck_id))
    {
        errors.push("expected_truck_not_found".to_string());
    }

    validate_graph_presence_and_refs(
        &parsed,
        expected_truck_id,
        &mut dangling_references,
        &mut errors,
    );

    if let (Some(driver_id), Some(driver_truck_id)) = (expected_driver_id, expected_driver_truck_id)
    {
        validate_graph_presence_and_refs(
            &parsed,
            driver_truck_id,
            &mut dangling_references,
            &mut errors,
        );
        let assigned_to_driver = parsed
            .garage_assignments
            .get(&normalize_sii_unit_id(driver_truck_id))
            .and_then(|assignment| assignment.driver_id.as_deref())
            .map(|actual| normalize_sii_unit_id(actual) == normalize_sii_unit_id(driver_id))
            .unwrap_or(false);
        if !assigned_to_driver {
            errors.push("driver_truck_assignment_mismatch".to_string());
        }

        let driver_declares_previous_truck = parsed
            .driver_infos
            .get(&normalize_sii_unit_id(driver_id))
            .and_then(|driver| driver.current_truck_id.as_deref())
            .map(|actual| normalize_sii_unit_id(actual) == normalize_sii_unit_id(driver_truck_id))
            .unwrap_or(true);
        if !driver_declares_previous_truck {
            errors.push("driver_assigned_truck_field_mismatch".to_string());
        }

        let player_target_has_driver = parsed
            .garage_assignments
            .get(&normalize_sii_unit_id(expected_truck_id))
            .and_then(|assignment| assignment.driver_id.as_deref())
            .is_some();
        if player_target_has_driver {
            errors.push("player_truck_still_assigned_to_driver".to_string());
        }

        if actual_truck_id
            .as_deref()
            .map(|actual| actual.eq_ignore_ascii_case(driver_truck_id))
            .unwrap_or(false)
        {
            errors.push("driver_truck_still_active".to_string());
        }
    }

    let duplicate_assignments = duplicate_assigned_trucks(&parsed);
    if !duplicate_assignments.is_empty() {
        errors.push("duplicate_assignment_detected".to_string());
    }
    if garage_driver_references_unresolved(&parsed) {
        errors.push("driver_assignment_unresolved".to_string());
    }
    if garage_drivers_without_trucks(&parsed) {
        errors.push("driver_without_truck".to_string());
    }
    dangling_references.sort();
    dangling_references.dedup();
    errors.sort();
    errors.dedup();

    TruckWriteValidation {
        success: errors.is_empty(),
        expected_truck_id: expected_truck_id.to_string(),
        actual_truck_id,
        dangling_references,
        errors,
    }
}

fn validate_graph_presence_and_refs(
    parsed: &super::parser::ParsedTruckSave,
    truck_id: &str,
    dangling_references: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    match parsed.truck_graphs.get(truck_id) {
        Some(graph) => {
            dangling_references.extend(graph_dangling_accessories(graph, &parsed.unit_ids));
            for reference in &graph.referenced_unit_ids {
                if reference.starts_with("_nameless.") && !parsed.unit_ids.contains(reference) {
                    dangling_references.push(reference.clone());
                }
            }
            if !dangling_references.is_empty() {
                errors.push("dangling_vehicle_references".to_string());
            }
        }
        None => errors.push("vehicle_block_not_found".to_string()),
    }
}

fn duplicate_assigned_trucks(parsed: &super::parser::ParsedTruckSave) -> Vec<String> {
    let blocks = parsed.unit_blocks.values().cloned().collect::<Vec<_>>();
    assignment_conflicts_from_blocks(&blocks)
}

fn garage_driver_references_unresolved(parsed: &super::parser::ParsedTruckSave) -> bool {
    parsed.garage_assignments.values().any(|assignment| {
        assignment
            .driver_id
            .as_deref()
            .map(|driver_id| {
                if parsed
                    .driver_infos
                    .contains_key(&normalize_sii_unit_id(driver_id))
                {
                    return false;
                }

                !is_valid_garage_driver_ref(driver_id)
                    || !garage_driver_ref_is_unique(parsed, driver_id, &assignment.truck_id)
            })
            .unwrap_or(false)
    })
}

fn garage_drivers_without_trucks(parsed: &super::parser::ParsedTruckSave) -> bool {
    parsed
        .unit_blocks
        .values()
        .filter(|block| block.unit_type == "garage")
        .any(|block| {
            let vehicles = super::parser::extract_array_entries(&block.raw_block, "vehicles")
                .into_iter()
                .collect::<std::collections::HashMap<_, _>>();
            let drivers = super::parser::extract_array_entries(&block.raw_block, "drivers");
            drivers.iter().any(|(index, driver_id)| {
                !super::parser::is_null_ref(driver_id)
                    && vehicles
                        .get(index)
                        .map(|truck_id| super::parser::is_null_ref(truck_id))
                        .unwrap_or(true)
            })
        })
}

#[cfg(test)]
mod tests {
    use super::validate_truck_switch_content;

    #[test]
    fn validation_requires_semantic_active_truck_match() {
        let content = r#"SiiNunit
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
        let validation = validate_truck_switch_content(content, "_nameless.truck.b", None, None);
        assert!(!validation.success);
        assert!(
            validation
                .errors
                .contains(&"active_truck_mismatch".to_string())
        );
    }

    #[test]
    fn validation_uses_assigned_vehicles_pointer_when_my_truck_is_null() {
        let content = r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 assigned_vehicles: _nameless.assigned.1
 trucks: 1
 trucks[0]: _nameless.truck.b
 my_truck: null
 assigned_truck: null
}
player_vehicles : _nameless.assigned.1 {
 vehicle: _nameless.truck.b
 trailer: null
}
vehicle : _nameless.truck.b {
 accessories: 1
 accessories[0]: _nameless.acc.b
}
vehicle_accessory : _nameless.acc.b {
 data_path: "/def/vehicle/truck/scania.s_2016/data.sii"
}
}
"#;
        let validation = validate_truck_switch_content(content, "_nameless.truck.b", None, None);

        assert!(validation.success, "{:?}", validation.errors);
        assert_eq!(
            validation.actual_truck_id.as_deref(),
            Some("_nameless.truck.b")
        );
    }

    #[test]
    fn validation_reports_dangling_accessory_reference() {
        let content = r#"SiiNunit
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
 accessories[0]: _nameless.acc.missing
}
}
"#;
        let validation = validate_truck_switch_content(content, "_nameless.truck.a", None, None);
        assert!(!validation.success);
        assert_eq!(
            validation.dangling_references,
            vec!["_nameless.acc.missing".to_string()]
        );
    }

    #[test]
    fn validation_reports_duplicate_driver_assignment() {
        let content = r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 my_truck: _nameless.truck.b
 trucks: 2
 trucks[0]: _nameless.truck.a
 trucks[1]: _nameless.truck.b
}
vehicle : _nameless.truck.a {
 accessories: 1
 accessories[0]: _nameless.acc.a
}
vehicle_accessory : _nameless.acc.a {
 data_path: "/def/vehicle/truck/scania.s_2016/data.sii"
}
vehicle : _nameless.truck.b {
 accessories: 1
 accessories[0]: _nameless.acc.b
}
vehicle_accessory : _nameless.acc.b {
 data_path: "/def/vehicle/truck/man.tgx/data.sii"
}
garage : garage.berlin {
 vehicles: 2
 vehicles[0]: _nameless.truck.a
 vehicles[1]: _nameless.truck.b
 drivers: 2
 drivers[0]: driver.1
 drivers[1]: driver.1
}
}
"#;
        let validation = validate_truck_switch_content(
            content,
            "_nameless.truck.b",
            Some("driver.1"),
            Some("_nameless.truck.a"),
        );
        assert!(!validation.success);
        assert!(
            validation
                .errors
                .contains(&"duplicate_assignment_detected".to_string())
        );
    }
}
