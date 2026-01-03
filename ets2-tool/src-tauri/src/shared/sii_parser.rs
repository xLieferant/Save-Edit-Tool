use crate::dev_log;
use crate::models::trucks::ParsedTruck;
use crate::models::trailers::{TrailerData, TrailerDefData};
use crate::shared::hex_float::{hex_to_float, parse_value_auto};
use regex::Regex;
use std::collections::HashMap;

/// Hilfsfunktionen jetzt alle public
pub fn extract_value(block: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"{}\s*:\s*"([^"]*)""#, key)).unwrap();
    re.captures(block)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

pub fn extract_string(block: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"{}\s*:\s*"([^"]*)""#, key)).ok()?;
    re.captures(block)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

pub fn extract_string_array(block: &str, key: &str) -> Vec<String> {
    let re = Regex::new(&format!(r#"{}\[\d+\]:\s*"([^"]*)""#, key)).unwrap();
    re.captures_iter(block).map(|c| c[1].to_string()).collect()
}

pub fn extract_raw<'a>(block: &'a str, key: &'a str) -> Option<&'a str> {
    let pattern = format!(r"{}\s*:\s*&([0-9a-fA-F]+)", key);
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(block)?;
    caps.get(1).map(|m| m.as_str())
}

pub fn extract_i64(block: &str, key: &str) -> Option<i64> {
    let re = Regex::new(&format!(r#"{}\s*:\s*([0-9]+)"#, key)).unwrap();
    re.captures(block)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
}

pub fn extract_f32(block: &str, key: &str) -> Option<f32> {
    let re = Regex::new(&format!(r#"{}\s*:\s*([0-9\.\-]+)"#, key)).unwrap();
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

/// Parsen von Trucks aus SII-Dateien
pub fn parse_trucks_from_sii(content: &str) -> Vec<ParsedTruck> {
    let mut trucks = Vec::new();

    let re_vehicle_accessory_block = Regex::new(r"vehicle_accessory\s*:\s*([^\s]+)\s*\{([^}]+)\}").unwrap();
    let re_data_path = Regex::new(r#"data_path:\s*"([^"]+)""#).unwrap();

    let mut accessory_map: HashMap<String, String> = HashMap::new();
    for cap in re_vehicle_accessory_block.captures_iter(content) {
        if let Some(dp_cap) = re_data_path.captures(&cap[2]) {
            accessory_map.insert(cap[1].to_string(), dp_cap[1].to_string());
        }
    }

    let re_block = Regex::new(r"(vehicle\s*:\s*[^\s]+)\s*\{([^}]+)\}").unwrap();
    for caps in re_block.captures_iter(content) {
        let truck_id_raw = caps.get(1).unwrap().as_str().trim().to_string();
        let truck_id = truck_id_raw
            .split(':')
            .nth(1)
            .unwrap_or("")
            .trim()
            .to_string();
        let block = caps.get(2).unwrap().as_str();

        let re_accessory = Regex::new(r"accessories\[\d+\]:\s*([^\s]+)").unwrap();
        let mut accessories = Vec::new();
        for acc_cap in re_accessory.captures_iter(block) {
            accessories.push(acc_cap[1].to_string());
        }

        let mut brand = String::new();
        let mut model = String::new();
        for acc in &accessories {
            if let Some(path) = accessory_map.get(acc) {
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() >= 5 {
                    let truck_info = parts[4];
                    let parts2: Vec<&str> = truck_info.split('.').collect();
                    brand = parts2.get(0).unwrap_or(&"").to_string();
                    model = parts2[1..].join(".");
                    break;
                }
            }
        }

        // Helper to parse arrays like wheels_wear[0], wheels_wear[1]...
        let parse_array = |key: &str| -> Vec<f32> {
            let re_arr = Regex::new(&format!(r"{}\[\d+\]:\s*(&[0-9a-fA-F]+|[0-9\.\-]+)", key)).unwrap();
            let mut vals = Vec::new();
            for cap in re_arr.captures_iter(block) {
                if let Ok(v) = parse_value_auto(&cap[1]) {
                    vals.push(v);
                }
            }
            vals
        };

        // Helper for single values that might be hex or float
        let parse_val = |key: &str| -> f32 {
            let re_val = Regex::new(&format!(r"{}\s*:\s*(&[0-9a-fA-F]+|[0-9\.\-]+)", key)).unwrap();
            re_val.captures(block)
                .and_then(|c| parse_value_auto(&c[1]).ok())
                .unwrap_or(0.0)
        };

        let odometer = extract_combined_float(block, "odometer");
        let integrity_odometer = extract_combined_float(block, "integrity_odometer");
        
        let fuel_relative = parse_val("fuel_relative");
        let trip_fuel_l = extract_combined_float(block, "trip_fuel"); // trip_fuel + trip_fuel_l logic usually combined or separate? SII usually has trip_fuel (hex) and trip_fuel_l (int). We use combined logic if pattern matches, else parse_val.
        // Actually trip_fuel is often the float part of trip_fuel_l or separate. Let's use combined for safety if keys exist.
        let trip_fuel_val = extract_combined_float(block, "trip_fuel"); // trip_fuel is often the key for the float part in some versions, or separate.
        // In the input: trip_fuel_l: 1128, trip_fuel: &3d8fa681. This looks like int + float part pattern but named differently.
        // Let's just sum them as floats.
        let trip_fuel_l_int = extract_f32(block, "trip_fuel_l").unwrap_or(0.0);
        let trip_fuel_hex = parse_val("trip_fuel");
        let trip_fuel_total = trip_fuel_l_int + trip_fuel_hex;

        let trip_distance_km = extract_f32(block, "trip_distance_km").unwrap_or(0.0) + parse_val("trip_distance");
        let trip_time_min = extract_f32(block, "trip_time_min").unwrap_or(0.0) + parse_val("trip_time");

        let license_plate = extract_value(block, "license_plate");
        let assigned_garage = extract_value(block, "assigned_garage");

        let engine_wear = parse_val("engine_wear");
        let transmission_wear = parse_val("transmission_wear");
        let cabin_wear = parse_val("cabin_wear");
        let chassis_wear = parse_val("chassis_wear");
        let wheels_wear = parse_array("wheels_wear");

        let engine_wear_unfixable = parse_val("engine_wear_unfixable");
        let transmission_wear_unfixable = parse_val("transmission_wear_unfixable");
        let cabin_wear_unfixable = parse_val("cabin_wear_unfixable");
        let chassis_wear_unfixable = parse_val("chassis_wear_unfixable");
        let wheels_wear_unfixable = parse_array("wheels_wear_unfixable");

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
    // trailer_def : _nameless.230.0ea6.67b0 { ... }
    let re_def = Regex::new(r"trailer_def\s*:\s*([a-zA-Z0-9._]+)\s*\{([^}]+)\}").unwrap();
    
    for cap in re_def.captures_iter(content) {
        let id = cap[1].to_string();
        let block = &cap[2];

        let parse_val = |key: &str| -> f32 {
            let re_val = Regex::new(&format!(r"{}\s*:\s*(&[0-9a-fA-F]+|[0-9\.\-]+)", key)).unwrap();
            re_val.captures(block)
                .and_then(|c| parse_value_auto(&c[1]).ok())
                .unwrap_or(0.0)
        };

        let data = TrailerDefData {
            id: id.clone(),
            gross_trailer_weight_limit: parse_val("gross_trailer_weight_limit"),
            chassis_mass: parse_val("chassis_mass"),
            body_mass: parse_val("body_mass"),
            length: parse_val("length"),
            body_type: extract_value(block, "body_type"),
            chain_type: extract_value(block, "chain_type"),
            source_name: extract_value(block, "source_name"),
        };
        defs.insert(id, data);
    }
    defs
}

/// Parsen von Trailers aus SII-Dateien
pub fn parse_trailers_from_sii(text: &str) -> Vec<TrailerData> {
    let mut trailers = Vec::new();

    let block_re = Regex::new(r#"trailer\s*:\s*([a-zA-Z0-9._]+)\s*\{(?P<body>.*?)\}"#).unwrap();
    let re_plate = Regex::new(r#"license_plate:\s*"([^"]+)""#).unwrap();
    let re_brand = Regex::new(r#"brand:\s*([a-zA-Z0-9/._-]+)"#).unwrap();
    let re_model = Regex::new(r#"model:\s*([a-zA-Z0-9/._-]+)"#).unwrap();
    let re_assigned = Regex::new(r#"assigned_garage:\s*([a-zA-Z0-9._-]+)"#).unwrap();
    let re_def_ref = Regex::new(r"trailer_definition\s*:\s*([a-zA-Z0-9._]+)").unwrap();

    for cap in block_re.captures_iter(text) {
        let trailer_id = cap[1].to_string();
        let body = cap.name("body").unwrap().as_str();

        let brand = re_brand.captures(body).map(|c| c[1].to_string());
        let model = re_model.captures(body).map(|c| c[1].to_string());
        let license_plate = re_plate.captures(body).map(|c| c[1].to_string());
        let trailer_definition = re_def_ref.captures(body).map(|c| c[1].to_string()).unwrap_or_default();

        let odometer = extract_combined_float(body, "odometer");
        let integrity_odometer = extract_combined_float(body, "integrity_odometer");

        let parse_val = |key: &str| -> f32 {
            let re_val = Regex::new(&format!(r"{}\s*:\s*(&[0-9a-fA-F]+|[0-9\.\-]+)", key)).unwrap();
            re_val.captures(body)
                .and_then(|c| parse_value_auto(&c[1]).ok())
                .unwrap_or(0.0)
        };

        let parse_array = |key: &str| -> Vec<f32> {
            let re_arr = Regex::new(&format!(r"{}\[\d+\]:\s*(&[0-9a-fA-F]+|[0-9\.\-]+)", key)).unwrap();
            let mut vals = Vec::new();
            for cap in re_arr.captures_iter(body) {
                if let Ok(v) = parse_value_auto(&cap[1]) {
                    vals.push(v);
                }
            }
            vals
        };

        let wear_float = Some(parse_val("trailer_body_wear")); // Legacy name in struct
        let body_wear_unfixable = parse_val("trailer_body_wear_unfixable");
        
        let chassis_wear = parse_val("chassis_wear");
        let chassis_wear_unfixable = parse_val("chassis_wear_unfixable");

        let wheels = parse_array("wheels_wear");
        let wheels_float = if wheels.is_empty() {
            None
        } else {
            Some(wheels)
        };
        let wheels_wear_unfixable = parse_array("wheels_wear_unfixable");

        let cargo_mass = parse_val("cargo_mass");
        let cargo_damage = parse_val("cargo_damage");
        
        let accessories = extract_string_array(body, "accessories");

        let assigned_garage = re_assigned.captures(body).map(|c| c[1].to_string());

        trailers.push(TrailerData {
            trailer_id,
            trailer_definition,
            brand,
            model,
            license_plate,
            odometer,
            odometer_float: None, // Already combined
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

    trailers
}

pub fn get_player_id(content: &str) -> Option<String> {
    let mut in_economy = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("economy :") {
            in_economy = true;
        }
        if in_economy && trimmed.starts_with("player:") {
            return trimmed.split_whitespace().nth(1).map(|s| s.to_string());
        }
        if in_economy && trimmed.starts_with('}') {
            in_economy = false;
        }
    }
    None
}

pub fn get_vehicle_ids(content: &str, player_id: &str) -> (Option<String>, Option<String>) {
    let mut in_player = false;
    let mut truck_id = None;
    let mut trailer_id = None;

    let player_block_start = format!("player : {}", player_id);

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&player_block_start) {
            in_player = true;
        }
        if in_player {
            if trimmed.starts_with("my_truck:") {
                truck_id = trimmed.split_whitespace().nth(1).map(|s| s.to_string());
            }
            if trimmed.starts_with("my_trailer:") {
                trailer_id = trimmed.split_whitespace().nth(1).map(|s| s.to_string());
            }
            if trimmed.starts_with("}") {
                break;
            }
        }
    }
    (truck_id, trailer_id)
}