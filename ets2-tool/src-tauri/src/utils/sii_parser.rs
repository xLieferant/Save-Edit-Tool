use crate::log;
use crate::models::trucks::ParsedTruck;
use regex::Regex;
use std::collections::HashMap;

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
        let truck_id = truck_id_raw.split(':').nth(1).unwrap_or("").trim().to_string();
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

        // EINFACHES LOG HIER:
        log!(
            "Parsed Truck Data -> ID: {}, Brand: {}, Model: {}, Odo: {:?}, Mileage: {:?}, Fuel: {:?}, Plate: {:?}, Garage: {:?}",
            truck_id,
            brand,
            model,
            odometer,
            mileage,
            trip_fuel_l,
            license_plate,
            assigned_garage
        );
        // ENDE DES LOGS

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
