use super::models::TruckWriteValidation;
use super::parser::{graph_dangling_accessories, parse_truck_save};

pub fn validate_truck_switch_content(
    content: &str,
    expected_truck_id: &str,
) -> TruckWriteValidation {
    let parsed = parse_truck_save(content);
    let actual_truck_id = parsed.active_truck_id.clone();
    let mut dangling_references = Vec::new();
    let mut errors = Vec::new();

    match actual_truck_id.as_deref() {
        Some(actual) if actual.eq_ignore_ascii_case(expected_truck_id) => {}
        Some(_) => errors.push("active_truck_mismatch".to_string()),
        None => errors.push("missing_my_truck_pointer".to_string()),
    }

    if !parsed
        .trucks
        .iter()
        .any(|truck| truck.truck_id.eq_ignore_ascii_case(expected_truck_id))
    {
        errors.push("expected_truck_not_found".to_string());
    }

    match parsed.truck_graphs.get(expected_truck_id) {
        Some(graph) => {
            dangling_references.extend(graph_dangling_accessories(graph, &parsed.unit_ids));
            for reference in &graph.referenced_unit_ids {
                if reference.starts_with("_nameless.") && !parsed.unit_ids.contains(reference) {
                    dangling_references.push(reference.clone());
                }
            }
            dangling_references.sort();
            dangling_references.dedup();
            if !dangling_references.is_empty() {
                errors.push("dangling_vehicle_references".to_string());
            }
        }
        None => errors.push("vehicle_block_not_found".to_string()),
    }

    TruckWriteValidation {
        success: errors.is_empty(),
        expected_truck_id: expected_truck_id.to_string(),
        actual_truck_id,
        dangling_references,
        errors,
    }
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
        let validation = validate_truck_switch_content(content, "_nameless.truck.b");
        assert!(!validation.success);
        assert!(
            validation
                .errors
                .contains(&"active_truck_mismatch".to_string())
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
        let validation = validate_truck_switch_content(content, "_nameless.truck.a");
        assert!(!validation.success);
        assert_eq!(
            validation.dangling_references,
            vec!["_nameless.acc.missing".to_string()]
        );
    }
}
