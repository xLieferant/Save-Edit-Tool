use std::path::Path;

use super::models::{PowertrainCatalog, PowertrainComponentPreview, TruckPowertrainPreview};
use super::parser::{
    graph_engine_data_path, graph_primary_family, graph_transmission_data_path, parse_truck_save,
};

pub fn load_official_powertrain_catalog(
    repo_root: &Path,
    game: &str,
    game_version: &str,
) -> Result<PowertrainCatalog, String> {
    let path = repo_root.join(format!(
        "data/vehicle/powertrain_catalog.{}.{}.json",
        game, game_version
    ));
    if !path.exists() {
        return Err(format!("powertrain_catalog_not_found:{}", path.display()));
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("powertrain_catalog_read_failed:{}", error))?;
    serde_json::from_str(&content).map_err(|error| format!("powertrain_catalog_invalid:{}", error))
}

pub fn preview_powertrain_change_from_content(
    content: &str,
    catalog: &PowertrainCatalog,
    truck_id: &str,
    engine_data_path: Option<&str>,
    transmission_data_path: Option<&str>,
) -> TruckPowertrainPreview {
    let parsed = parse_truck_save(content);
    let graph = parsed.truck_graphs.get(truck_id);
    let truck_family = graph.and_then(graph_primary_family);
    let current_engine_path = graph.and_then(graph_engine_data_path);
    let current_transmission_path = graph.and_then(graph_transmission_data_path);
    let selected_engine = engine_data_path.and_then(|path| {
        catalog
            .engines
            .iter()
            .find(|engine| same_data_path(&engine.data_path, path))
    });
    let selected_transmission = transmission_data_path.and_then(|path| {
        catalog
            .transmissions
            .iter()
            .find(|transmission| same_data_path(&transmission.data_path, path))
    });
    let selected_engine_family =
        selected_engine.map(|engine| component_family(&engine.brand, &engine.truck_model));
    let selected_transmission_family = selected_transmission
        .map(|transmission| component_family(&transmission.brand, &transmission.truck_model));
    let engine_same_family = selected_engine_family.as_deref().and_then(|family| {
        truck_family
            .as_deref()
            .map(|truck| same_family(truck, family))
    });
    let transmission_same_family = selected_transmission_family.as_deref().and_then(|family| {
        truck_family
            .as_deref()
            .map(|truck| same_family(truck, family))
    });
    let mut warnings = Vec::new();

    if graph.is_none() {
        warnings.push("truck_graph_missing".to_string());
    }
    if current_engine_path.is_none() {
        warnings.push("missing_engine_accessory_block".to_string());
    }
    if current_transmission_path.is_none() {
        warnings.push("missing_transmission_accessory_block".to_string());
    }
    if engine_data_path.is_some() && selected_engine.is_none() {
        warnings.push("engine_definition_missing".to_string());
    }
    if transmission_data_path.is_some() && selected_transmission.is_none() {
        warnings.push("transmission_definition_missing".to_string());
    }
    let experimental_cross_brand =
        engine_same_family == Some(false) || transmission_same_family == Some(false);
    if experimental_cross_brand {
        warnings.push("experimental_cross_brand".to_string());
    }
    let can_apply_later = graph.is_some()
        && current_engine_path.is_some()
        && current_transmission_path.is_some()
        && engine_data_path
            .map(|_| selected_engine.is_some())
            .unwrap_or(true)
        && transmission_data_path
            .map(|_| selected_transmission.is_some())
            .unwrap_or(true);

    TruckPowertrainPreview {
        truck_id: truck_id.to_string(),
        truck_family,
        current_engine: PowertrainComponentPreview {
            current_data_path: current_engine_path,
            selected_data_path: None,
            selected_name: None,
            selected_family: None,
        },
        new_engine: engine_data_path.map(|path| PowertrainComponentPreview {
            current_data_path: None,
            selected_data_path: Some(path.to_string()),
            selected_name: selected_engine.map(|engine| engine.name.clone()),
            selected_family: selected_engine_family,
        }),
        current_transmission: PowertrainComponentPreview {
            current_data_path: current_transmission_path,
            selected_data_path: None,
            selected_name: None,
            selected_family: None,
        },
        new_transmission: transmission_data_path.map(|path| PowertrainComponentPreview {
            current_data_path: None,
            selected_data_path: Some(path.to_string()),
            selected_name: selected_transmission.map(|transmission| transmission.name.clone()),
            selected_family: selected_transmission_family,
        }),
        selected_differential_ratio: selected_transmission.and_then(|item| item.differential_ratio),
        engine_same_family,
        transmission_same_family,
        experimental_cross_brand,
        warnings,
        can_apply_later,
    }
}

fn same_data_path(left: &str, right: &str) -> bool {
    left.replace('\\', "/")
        .eq_ignore_ascii_case(&right.replace('\\', "/"))
}

fn component_family(brand: &str, model: &str) -> String {
    if model.trim().is_empty() {
        brand.to_string()
    } else {
        format!("{}.{}", brand, model)
    }
}

fn same_family(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[cfg(test)]
mod tests {
    use super::preview_powertrain_change_from_content;
    use crate::features::truck_change::models::{
        PowertrainCatalog, PowertrainEngine, PowertrainTransmission,
    };

    fn catalog() -> PowertrainCatalog {
        PowertrainCatalog {
            schema_version: 1,
            game: "ets2".to_string(),
            game_version: "test".to_string(),
            generated_at: String::new(),
            sources: vec![],
            engines: vec![PowertrainEngine {
                id: "scania_engine".to_string(),
                data_path: "/def/vehicle/truck/scania.s_2016/engine/dc16.sii".to_string(),
                brand: "scania".to_string(),
                truck_model: "s_2016".to_string(),
                name: "DC16".to_string(),
                engine_type: "diesel".to_string(),
                torque_nm: None,
                power: None,
                rpm_idle: None,
                rpm_limit: None,
                official: true,
                source_archive: "def.scs".to_string(),
            }],
            transmissions: vec![PowertrainTransmission {
                id: "scania_transmission".to_string(),
                data_path: "/def/vehicle/truck/scania.s_2016/transmission/g33.sii".to_string(),
                brand: "scania".to_string(),
                truck_model: "s_2016".to_string(),
                name: "G33".to_string(),
                gears_forward: Some(12),
                ratios_forward: vec![],
                ratios_reverse: vec![],
                differential_ratio: Some(2.59),
                retarder_steps: None,
                official: true,
                source_archive: "def.scs".to_string(),
            }],
        }
    }

    #[test]
    fn powertrain_preview_reads_differential_from_catalog() {
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
 accessories: 2
 accessories[0]: _nameless.acc.engine
 accessories[1]: _nameless.acc.transmission
}
vehicle_accessory : _nameless.acc.engine {
 data_path: "/def/vehicle/truck/scania.s_2016/engine/old.sii"
}
vehicle_accessory : _nameless.acc.transmission {
 data_path: "/def/vehicle/truck/scania.s_2016/transmission/old.sii"
}
}
"#;
        let preview = preview_powertrain_change_from_content(
            content,
            &catalog(),
            "_nameless.truck.a",
            None,
            Some("/def/vehicle/truck/scania.s_2016/transmission/g33.sii"),
        );
        assert_eq!(preview.selected_differential_ratio, Some(2.59));
        assert!(preview.can_apply_later);
    }
}
