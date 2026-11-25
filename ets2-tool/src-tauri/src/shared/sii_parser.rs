use crate::dev_log;
use crate::models::trailers::{TrailerData, TrailerDefData};
use crate::models::trucks::ParsedTruck;
use crate::shared::hex_float::{hex_to_float, parse_value_auto};
use regex::Regex;
use std::collections::HashMap;

fn compile_regex(pattern: &str, context: &str) -> Option<Regex> {
    match Regex::new(pattern) {
        Ok(regex) => Some(regex),
        Err(error) => {
            dev_log!(
                "[sii_parser] regex compile failed in {}: {}",
                context,
                error
            );
            None
        }
    }
}

fn extract_numeric_value_auto(block: &str, key: &str) -> f32 {
    let pattern = format!(
        r"(?m)^[ \t]*{}[ \t]*:[ \t]*(&[0-9a-fA-F]+|[0-9.eE+\-]+)",
        regex::escape(key)
    );
    let Some(regex) = compile_regex(&pattern, "extract_numeric_value_auto") else {
        return 0.0;
    };

    regex
        .captures(block)
        .and_then(|captures| captures.get(1))
        .and_then(|value| parse_value_auto(value.as_str()).ok())
        .unwrap_or(0.0)
}

fn extract_numeric_array_auto(block: &str, key: &str) -> Vec<f32> {
    let pattern = format!(
        r"{}\[\d+\]:\s*(&[0-9a-fA-F]+|[0-9\.\-]+)",
        regex::escape(key)
    );
    let Some(regex) = compile_regex(&pattern, "extract_numeric_array_auto") else {
        return Vec::new();
    };

    regex
        .captures_iter(block)
        .filter_map(|captures| captures.get(1))
        .filter_map(|value| parse_value_auto(value.as_str()).ok())
        .collect()
}

/// Hilfsfunktionen jetzt alle public
pub fn extract_value(block: &str, key: &str) -> Option<String> {
    let re = compile_regex(&format!(r#"{}\s*:\s*"([^"]*)""#, key), "extract_value")?;
    re.captures(block)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

pub fn extract_string(block: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"{}\s*:\s*"([^"]*)""#, key)).ok()?;
    re.captures(block)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

pub fn extract_string_array(block: &str, key: &str) -> Vec<String> {
    let Some(re) = compile_regex(
        &format!(r#"{}\[\d+\]:\s*"([^"]*)""#, key),
        "extract_string_array",
    ) else {
        return Vec::new();
    };
    re.captures_iter(block).map(|c| c[1].to_string()).collect()
}

pub fn extract_raw<'a>(block: &'a str, key: &'a str) -> Option<&'a str> {
    let pattern = format!(r"{}\s*:\s*&([0-9a-fA-F]+)", key);
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(block)?;
    caps.get(1).map(|m| m.as_str())
}

pub fn extract_i64(block: &str, key: &str) -> Option<i64> {
    let re = compile_regex(&format!(r#"{}\s*:\s*([0-9]+)"#, key), "extract_i64")?;
    re.captures(block)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
}

pub fn extract_f32(block: &str, key: &str) -> Option<f32> {
    let re = compile_regex(
        &format!(r#"(?m)^[ \t]*{}[ \t]*:[ \t]*([0-9.eE+\-]+)"#, key),
        "extract_f32",
    )?;
    re.captures(block)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f32>().ok())
}

/// Kombiniert einen Basiswert (Int/Float) und einen Hex-Float-Part
/// z.B. odometer: 37619 + odometer_float_part: &3f0388f8
fn extract_combined_float(block: &str, key_base: &str) -> f32 {
    let val_base = extract_f32(block, key_base).unwrap_or(0.0);
    let key_float = format!("{}_float_part", key_base);
    let val_float = extract_raw(block, &key_float)
        .and_then(|h| hex_to_float(h).ok())
        .unwrap_or(0.0);

    val_base + val_float
}

// Helper function to extract blocks with proper brace matching
fn extract_blocks_with_braces(content: &str, block_type: &str) -> Vec<(String, String)> {
    crate::shared::sii_document::scan_units(content, false)
        .unwrap_or_default()
        .into_iter()
        .filter(|unit| unit.kind == block_type)
        .map(|unit| (unit.id, content[unit.body].to_string()))
        .collect()
}

/// Extrahiert einen einzelnen, namentlich referenzierten Block (z.B. "player : _nameless.xyz { ... }")
/// mittels korrektem Brace-Matching. Nötig, weil einzelne Blöcke (player, player_vehicles, vehicle)
/// gezielt über ihre ID aufgelöst werden müssen, statt nur "irgendein Block dieses Typs".
fn extract_named_block<'a>(content: &'a str, block_type: &str, id: &str) -> Option<&'a str> {
    let units = crate::shared::sii_document::scan_units(content, false).ok()?;
    let mut matches = units
        .iter()
        .filter(|unit| unit.kind == block_type && unit.id == id);
    let unit = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(&content[unit.body.clone()])
}

/// Liest den Wert eines simplen "key: value"-Feldes direkt im übergebenen Block
/// (nicht rekursiv in Unterblöcke). Gibt None zurück, wenn der Key fehlt oder der Wert "null" ist.
/// Nutzt Zeilen-Matching mit ":" direkt nach dem Key, damit z.B. "vehicle:" nicht mit
/// "stored_vehicle_placement:" kollidiert.
fn extract_field_value(block: &str, key: &str) -> Option<String> {
    let prefix = format!("{}:", key);
    for line in block.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            let value = rest.trim().split_whitespace().next()?.to_string();
            if value == "null" || value.is_empty() {
                return None;
            }
            return Some(value);
        }
    }
    None
}

/// Parsen von Trucks aus SII-Dateien
pub fn parse_trucks_from_sii(content: &str) -> Vec<ParsedTruck> {
    let mut trucks = Vec::new();

    let Some(re_vehicle_accessory_block) = compile_regex(
        r"vehicle_accessory\s*:\s*([^\s]+)\s*\{([^}]+)\}",
        "parse_trucks_from_sii.vehicle_accessory",
    ) else {
        return trucks;
    };
    let Some(re_data_path) = compile_regex(
        r#"data_path:\s*"([^"]+)""#,
        "parse_trucks_from_sii.data_path",
    ) else {
        return trucks;
    };
    let Some(re_accessory) = compile_regex(
        r"accessories\[\d+\]:\s*([^\s]+)",
        "parse_trucks_from_sii.accessories",
    ) else {
        return trucks;
    };

    let mut accessory_map: HashMap<String, String> = HashMap::new();
    for cap in re_vehicle_accessory_block.captures_iter(content) {
        let Some(accessory_id) = cap.get(1) else {
            continue;
        };
        let Some(accessory_body) = cap.get(2) else {
            continue;
        };
        if let Some(dp_cap) = re_data_path.captures(accessory_body.as_str()) {
            if let Some(data_path) = dp_cap.get(1) {
                accessory_map.insert(
                    accessory_id.as_str().to_string(),
                    data_path.as_str().to_string(),
                );
            }
        }
    }

    let vehicle_blocks = extract_blocks_with_braces(content, "vehicle");

    for (truck_id, block) in vehicle_blocks {
        let mut accessories = Vec::new();
        for acc_cap in re_accessory.captures_iter(&block) {
            if let Some(accessory_id) = acc_cap.get(1) {
                accessories.push(accessory_id.as_str().to_string());
            }
        }

        let mut brand = String::new();
        let mut model = String::new();
        for acc in &accessories {
            if let Some(path) = accessory_map.get(acc) {
                if let Some(truck_info) = path
                    .strip_prefix("/def/vehicle/truck/")
                    .and_then(|rest| rest.split('/').next())
                {
                    let (brand_token, model_token) =
                        truck_info.split_once('.').unwrap_or((truck_info, ""));
                    brand = brand_token.to_string();
                    model = model_token.to_string();
                    break;
                }
            }
        }

        let odometer = extract_combined_float(&block, "odometer");
        let integrity_odometer = extract_combined_float(&block, "integrity_odometer");

        let fuel_relative = extract_numeric_value_auto(&block, "fuel_relative");

        let trip_fuel_l_int = extract_f32(&block, "trip_fuel_l").unwrap_or(0.0);
        let trip_fuel_hex = extract_numeric_value_auto(&block, "trip_fuel");
        let trip_fuel_total = trip_fuel_l_int + trip_fuel_hex;

        let trip_distance_km = extract_f32(&block, "trip_distance_km").unwrap_or(0.0)
            + extract_numeric_value_auto(&block, "trip_distance");
        let trip_time_min = extract_f32(&block, "trip_time_min").unwrap_or(0.0)
            + extract_numeric_value_auto(&block, "trip_time");

        let license_plate = extract_value(&block, "license_plate");
        let assigned_garage = extract_value(&block, "assigned_garage");

        let engine_wear = extract_numeric_value_auto(&block, "engine_wear");
        let transmission_wear = extract_numeric_value_auto(&block, "transmission_wear");
        let cabin_wear = extract_numeric_value_auto(&block, "cabin_wear");
        let chassis_wear = extract_numeric_value_auto(&block, "chassis_wear");
        let wheels_wear = extract_numeric_array_auto(&block, "wheels_wear");

        let engine_wear_unfixable = extract_numeric_value_auto(&block, "engine_wear_unfixable");
        let transmission_wear_unfixable =
            extract_numeric_value_auto(&block, "transmission_wear_unfixable");
        let cabin_wear_unfixable = extract_numeric_value_auto(&block, "cabin_wear_unfixable");
        let chassis_wear_unfixable = extract_numeric_value_auto(&block, "chassis_wear_unfixable");
        let wheels_wear_unfixable = extract_numeric_array_auto(&block, "wheels_wear_unfixable");

        trucks.push(ParsedTruck {
            truck_id,
            brand,
            model,
            odometer,
            integrity_odometer,
            fuel_relative,
            trip_fuel_l: trip_fuel_total,
            trip_distance_km,
            trip_time_min,
            engine_wear,
            transmission_wear,
            cabin_wear,
            chassis_wear,
            wheels_wear,
            engine_wear_unfixable,
            transmission_wear_unfixable,
            cabin_wear_unfixable,
            chassis_wear_unfixable,
            wheels_wear_unfixable,
            license_plate,
            assigned_garage,
        });
    }

    trucks
}

/// Parsen der Trailer-Definitionen (für Massen, Body-Type etc.)
pub fn parse_trailer_defs_from_sii(content: &str) -> HashMap<String, TrailerDefData> {
    let mut defs = HashMap::new();

    let def_blocks = extract_blocks_with_braces(content, "trailer_def");

    for (id, block) in def_blocks {
        let data = TrailerDefData {
            id: id.clone(),
            gross_trailer_weight_limit: extract_numeric_value_auto(
                &block,
                "gross_trailer_weight_limit",
            ),
            chassis_mass: extract_numeric_value_auto(&block, "chassis_mass"),
            body_mass: extract_numeric_value_auto(&block, "body_mass"),
            length: extract_numeric_value_auto(&block, "length"),
            body_type: extract_value(&block, "body_type"),
            chain_type: extract_value(&block, "chain_type"),
            source_name: extract_value(&block, "source_name"),
        };
        defs.insert(id, data);
    }
    defs
}

/// Parsen von Trailers aus SII-Dateien
pub fn parse_trailers_from_sii(text: &str) -> Vec<TrailerData> {
    let mut trailers = Vec::new();

    let Some(re_plate) = compile_regex(
        r#"license_plate:\s*"([^"]+)""#,
        "parse_trailers_from_sii.license_plate",
    ) else {
        return trailers;
    };
    let Some(re_brand) = compile_regex(
        r#"brand:\s*([a-zA-Z0-9/._-]+)"#,
        "parse_trailers_from_sii.brand",
    ) else {
        return trailers;
    };
    let Some(re_model) = compile_regex(
        r#"model:\s*([a-zA-Z0-9/._-]+)"#,
        "parse_trailers_from_sii.model",
    ) else {
        return trailers;
    };
    let Some(re_assigned) = compile_regex(
        r#"assigned_garage:\s*([a-zA-Z0-9._-]+)"#,
        "parse_trailers_from_sii.assigned_garage",
    ) else {
        return trailers;
    };
    let Some(re_def_ref) = compile_regex(
        r"trailer_definition\s*:\s*([a-zA-Z0-9._]+)",
        "parse_trailers_from_sii.trailer_definition",
    ) else {
        return trailers;
    };

    let trailer_blocks = extract_blocks_with_braces(text, "trailer");

    for (trailer_id, body) in trailer_blocks {
        dev_log!("Parsing trailer: {}", trailer_id);

        let brand = re_brand
            .captures(&body)
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()));
        let model = re_model
            .captures(&body)
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()));
        let license_plate = re_plate
            .captures(&body)
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()));
        let trailer_definition = re_def_ref
            .captures(&body)
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()))
            .unwrap_or_default();

        let odometer = extract_combined_float(&body, "odometer");
        let integrity_odometer = extract_combined_float(&body, "integrity_odometer");

        let wear_float = Some(extract_numeric_value_auto(&body, "trailer_body_wear"));
        let body_wear_unfixable = extract_numeric_value_auto(&body, "trailer_body_wear_unfixable");

        let chassis_wear = extract_numeric_value_auto(&body, "chassis_wear");
        let chassis_wear_unfixable = extract_numeric_value_auto(&body, "chassis_wear_unfixable");

        let wheels = extract_numeric_array_auto(&body, "wheels_wear");
        let wheels_float = if wheels.is_empty() {
            None
        } else {
            Some(wheels)
        };
        let wheels_wear_unfixable = extract_numeric_array_auto(&body, "wheels_wear_unfixable");

        let cargo_mass = extract_numeric_value_auto(&body, "cargo_mass");
        let cargo_damage = extract_numeric_value_auto(&body, "cargo_damage");

        let accessories = extract_string_array(&body, "accessories");

        let assigned_garage = re_assigned
            .captures(&body)
            .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()));

        trailers.push(TrailerData {
            trailer_id,
            trailer_definition,
            brand,
            model,
            license_plate,
            odometer,
            odometer_float: None,
            wear_float,
            wheels_float,
            assigned_garage,
            cargo_mass,
            cargo_damage,
            body_wear_unfixable,
            chassis_wear,
            chassis_wear_unfixable,
            wheels_wear_unfixable,
            integrity_odometer,
            accessories,
        });
    }

    dev_log!("Total trailers parsed: {}", trailers.len());
    trailers
}

pub fn get_player_id(content: &str) -> Option<String> {
    let units = crate::shared::sii_document::scan_units(content, false).ok()?;
    let mut economies = units.iter().filter(|unit| unit.kind == "economy");
    let economy = economies.next()?;
    if economies.next().is_some() {
        return None;
    }
    extract_field_value(&content[economy.body.clone()], "player")
}

/// Löst Truck- und Trailer-ID des Spielers auf.
///
/// WICHTIG: `player.assigned_vehicles` zeigt NICHT direkt auf einen `vehicle`-Block,
/// sondern auf einen `player_vehicles`-Container. Der eigentliche Truck steckt erst
/// in dessen `vehicle:`-Feld, der Trailer in dessen `trailer:`-Feld.
/// `player.my_truck` / `player.my_trailer` sind in aktuellen Saves oft `null` und
/// dürfen nicht als primäre Quelle verwendet werden.
pub fn get_vehicle_ids(content: &str, player_id: &str) -> (Option<String>, Option<String>) {
    let Some(player_block) = extract_named_block(content, "player", player_id) else {
        dev_log!("[sii_parser] player block for {} not found", player_id);
        return (None, None);
    };

    let assigned_vehicles_ref = extract_field_value(player_block, "assigned_vehicles");

    let direct_truck = extract_field_value(player_block, "my_truck");
    let direct_trailer = extract_field_value(player_block, "my_trailer");
    let Some(pv_id) = assigned_vehicles_ref else {
        return (direct_truck, direct_trailer);
    };
    let Some(pv_block) = extract_named_block(content, "player_vehicles", &pv_id) else {
        return (None, None);
    };
    let truck_id = extract_field_value(pv_block, "vehicle").or(direct_truck);
    let trailer_id = extract_field_value(pv_block, "trailer").or(direct_trailer);

    (truck_id, trailer_id)
}
