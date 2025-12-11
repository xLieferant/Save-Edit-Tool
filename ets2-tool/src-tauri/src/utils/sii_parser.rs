use crate::log;
use crate::models::trucks::ParsedTruck;
use crate::models::trailers::ParsedTrailer;
use regex::Regex;
use std::collections::HashMap;

pub fn parse_trucks_from_sii(content: &str) -> Vec<ParsedTruck> {
    let mut trucks = Vec::new();

    // Mapping vehicle_accessory -> data_path
    let re_vehicle_accessory =
        Regex::new(r#"vehicle_accessory\s*:\s*([^\s]+)\s*\{\s*data_path:\s*"([^"]+)""#).unwrap();
    let mut accessory_map: HashMap<String, String> = HashMap::new();
    for cap in re_vehicle_accessory.captures_iter(content) {
        accessory_map.insert(cap[1].to_string(), cap[2].to_string());
    }

    // Truck Vehicle Blocks
    let re_block = Regex::new(r"(vehicle\s*:\s*[^\s]+)\s*\{([^}]+)\}").unwrap();
    for caps in re_block.captures_iter(content) {
        let truck_id_raw = caps.get(1).unwrap().as_str().trim().to_string();
        let truck_id = truck_id_raw.split(':').nth(1).unwrap_or("").trim().to_string();
        let block = caps.get(2).unwrap().as_str();

        // Accessories
        let re_accessory = Regex::new(r"accessories\[\d+\]:\s*([^\s]+)").unwrap();
        let mut accessories = Vec::new();
        for acc_cap in re_accessory.captures_iter(block) {
            accessories.push(acc_cap[1].to_string());
        }

        // Marke & Modell
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

        log!(
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

pub fn parse_trailers_from_sii(content: &str) -> Vec<ParsedTrailer> {
    let mut trailers = Vec::new();

    // Mapping vehicle_accessory -> data_path
    let re_vehicle_accessory =
        Regex::new(r#"vehicle_accessory\s*:\s*([^\s]+)\s*\{\s*data_path:\s*"([^"]+)""#).unwrap();
    let mut accessory_map: HashMap<String, String> = HashMap::new();
    for cap in re_vehicle_accessory.captures_iter(content) {
        accessory_map.insert(cap[1].to_string(), cap[2].to_string());
    }

    // Trailer Vehicle Blocks
    let re_block = Regex::new(r"(vehicle\s*:\s*[^\s]+)\s*\{([^}]+)\}").unwrap();
    for caps in re_block.captures_iter(content) {
        let raw_id = caps.get(1).unwrap().as_str().trim().to_string();
        let trailer_id = raw_id.split(':').nth(1).unwrap_or("").trim().to_string();
        let block = caps.get(2).unwrap().as_str();

        // Accessories
        let re_accessory = Regex::new(r"accessories\[\d+\]:\s*([^\s]+)").unwrap();
        let mut accessories = Vec::new();
        for acc_cap in re_accessory.captures_iter(block) {
            accessories.push(acc_cap[1].to_string());
        }

        // Marke & Modell
        let mut brand = None;
        let mut model = None;
        for acc in &accessories {
            if let Some(path) = accessory_map.get(acc) {
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() >= 5 {
                    let trailer_info = parts[4];
                    let split: Vec<&str> = trailer_info.split('.').collect();
                    if split.len() >= 2 {
                        brand = Some(split[0].to_string());
                        model = Some(split[1..].join("."));
                    }
                    break;
                }
            }
        }

        let odometer = extract_i64(block, "odometer");
        let odometer_float = extract_f32(block, "odometer_float");
        let wear_float = extract_f32(block, "chassis_wear");
        let wheels_float = extract_f32(block, "wheels_wear");
        let license_plate = extract_value(block, "license_plate");
        let assigned_garage = extract_value(block, "assigned_garage");

        log!(
            "Parsed Trailer -> ID: {}, Brand: {:?}, Model: {:?}, Odo: {:?}, Odo FLOAT: {:?}, Wear: {:?}, Wheels: {:?}, Plate: {:?}, Garage: {:?}",
            trailer_id,
            brand,
            model,
            odometer,
            odometer_float,
            wear_float,
            wheels_float,
            license_plate,
            assigned_garage
        );

        trailers.push(ParsedTrailer {
            trailer_id,
            brand,
            model,
            odometer,
            odometer_float,
            wear_float,
            wheels_float,
            license_plate,
            assigned_garage,
        });
    }

    trailers
}

fn extract_value(block: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"{}\s*:\s*"([^"]*)""#, key)).unwrap();
    re.captures(block)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

fn extract_i64(block: &str, key: &str) -> Option<i64> {
    let re = Regex::new(&format!(r#"{}\s*:\s*([0-9]+)"#, key)).unwrap();
    re.captures(block)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
}

fn extract_f32(block: &str, key: &str) -> Option<f32> {
    let re = Regex::new(&format!(r#"{}\s*:\s*([0-9\.\-]+)"#, key)).unwrap();
    re.captures(block)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f32>().ok())
}
