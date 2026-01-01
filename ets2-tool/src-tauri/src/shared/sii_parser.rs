use crate::dev_log;
use crate::models::trucks::ParsedTruck;
use crate::shared::hex_float::hex_to_float;
use regex::Regex;
use std::collections::HashMap;

/// Typ für Trailer-Daten
#[derive(Debug, Clone)]
pub struct TrailerData {
    pub trailer_id: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub license_plate: Option<String>,
    pub odometer: f32,
    pub odometer_float: Option<f32>,
    pub wear_float: Option<f32>,
    pub wheels_float: Option<Vec<f32>>,
    pub assigned_garage: Option<String>,
}

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

/// Parsen von Trucks aus SII-Dateien
pub fn parse_trucks_from_sii(content: &str) -> Vec<ParsedTruck> {
    let mut trucks = Vec::new();

    let re_vehicle_accessory =
        Regex::new(r#"vehicle_accessory\s*:\s*([^\s]+)\s*\{\s*data_path:\s*"([^"]+)""#).unwrap();
    let mut accessory_map: HashMap<String, String> = HashMap::new();
    for cap in re_vehicle_accessory.captures_iter(content) {
        accessory_map.insert(cap[1].to_string(), cap[2].to_string());
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

        let odometer = extract_i64(block, "odometer");
        let trip_fuel_l = extract_i64(block, "trip_fuel_l");
        let license_plate = extract_value(block, "license_plate");
        let mileage = extract_f32(block, "mileage");
        let assigned_garage = extract_value(block, "assigned_garage");

        dev_log!(
            "Parsed Truck -> ID: {}, Brand: {}, Model: {}, Odo: {:?}, Mileage: {:?}, Fuel: {:?}, Plate: {:?}, Garage: {:?}",
            truck_id,
            brand,
            model,
            odometer,
            mileage,
            trip_fuel_l,
            license_plate,
            assigned_garage
        );

        trucks.push(ParsedTruck {
            truck_id,
            brand,
            model,
            odometer,
            mileage,
            trip_fuel_l,
            license_plate,
            assigned_garage,
        });
    }

    trucks
}

/// Parsen von Trailers aus SII-Dateien
pub fn parse_trailers_from_sii(text: &str) -> Vec<TrailerData> {
    let mut trailers = Vec::new();

    let block_re = Regex::new(r#"trailer\s*:\s*([a-zA-Z0-9._]+)\s*\{(?P<body>.*?)\}"#).unwrap();
    let re_plate = Regex::new(r#"license_plate:\s*"([^"]+)""#).unwrap();
    let re_brand = Regex::new(r#"brand:\s*([a-zA-Z0-9/._-]+)"#).unwrap();
    let re_model = Regex::new(r#"model:\s*([a-zA-Z0-9/._-]+)"#).unwrap();
    let re_odometer = Regex::new(r#"odometer:\s*([0-9.]+)"#).unwrap();
    let re_odometer_float = Regex::new(r#"odometer_float_part:\s*(&[0-9a-fA-F]+)"#).unwrap();
    let re_body_wear = Regex::new(r#"trailer_body_wear:\s*(&[0-9a-fA-F]+)"#).unwrap();
    let re_wheel_entry = Regex::new(r#"wheels_wear\[(\d+)\]:\s*(&[0-9a-fA-F]+)"#).unwrap();
    let re_assigned = Regex::new(r#"assigned_garage:\s*([a-zA-Z0-9._-]+)"#).unwrap();

    for cap in block_re.captures_iter(text) {
        let trailer_id = cap[1].to_string();
        let body = cap.name("body").unwrap().as_str();

        let brand = re_brand.captures(body).map(|c| c[1].to_string());
        let model = re_model.captures(body).map(|c| c[1].to_string());
        let license_plate = re_plate.captures(body).map(|c| c[1].to_string());

        let odometer = re_odometer
            .captures(body)
            .and_then(|c| c[1].parse::<f32>().ok())
            .unwrap_or(0.0);

        let odometer_float = re_odometer_float
            .captures(body)
            .and_then(|c| hex_to_float(&c[1]).ok());

        let wear_float = re_body_wear
            .captures(body)
            .and_then(|c| hex_to_float(&c[1]).ok());

        let mut wheels = Vec::new();
        for wheel_cap in re_wheel_entry.captures_iter(body) {
            if let Ok(val) = hex_to_float(&wheel_cap[2]) {
                wheels.push(val);
            }
        }
        let wheels_float = if wheels.is_empty() {
            None
        } else {
            Some(wheels)
        };

        let assigned_garage = re_assigned.captures(body).map(|c| c[1].to_string());

        trailers.push(TrailerData {
            trailer_id,
            brand,
            model,
            license_plate,
            odometer,
            odometer_float,
            wear_float,
            wheels_float,
            assigned_garage,
        });
    }

    trailers
}
