use regex::Regex;
use crate::models::trucks::ParsedTruck;
use crate::log;
use std::collections::HashMap;

pub fn parse_trucks_from_sii(content: &str) -> Vec<ParsedTruck> {
    let mut trucks = Vec::new();

    log!("Starte Truck-Parsing");

    // 1) Alle vehicle_accessory-Einträge für data_path sammeln
    let re_vehicle_accessory =
        Regex::new(r#"vehicle_accessory\s*:\s*([^\s]+)\s*\{\s*data_path:\s*"([^"]+)""#).unwrap();

    // Map: accessory_id -> data_path
    let mut accessory_map: HashMap<String, String> = HashMap::new();
    for cap in re_vehicle_accessory.captures_iter(content) {
        let accessory_id = cap[1].to_string();
        let data_path = cap[2].to_string();
        accessory_map.insert(accessory_id, data_path);
    }

    // 2) Fahrzeug-Blöcke erkennen
    let re_block = Regex::new(r"(vehicle\s*:\s*[^\s]+)\s*\{([^}]+)\}").unwrap();

    for caps in re_block.captures_iter(content) {
        let truck_id = caps.get(1).unwrap().as_str().trim().to_string();
        let block = caps.get(2).unwrap().as_str();

        log!("Truck_ID gefunden: {}", truck_id);

        // Accessories sammeln
        let re_accessory = Regex::new(r"accessories\[\d+\]:\s*([^\s]+)").unwrap();
        let mut accessories = Vec::new();
        for acc_cap in re_accessory.captures_iter(block) {
            accessories.push(acc_cap[1].to_string());
        }

        // Marke und Modell über Accessories ermitteln
        let mut brand = String::new();
        let mut model = String::new();
        for acc in &accessories {
            if let Some(path) = accessory_map.get(acc) {
                // Beispiel: "/def/vehicle/truck/scania.s_2016/data.sii"
                let parts: Vec<&str> = path.split('/').collect();
                if parts.len() >= 5 {
                    let truck_info = parts[4]; // scania.s_2016
                    let mut split = truck_info.split('.');
                    brand = split.next().unwrap_or("").to_string();
                    model = split.next().unwrap_or("").to_string();
                    break; // ersten passenden Accessory nehmen
                }
            }
        }

        let garage = extract_value(block, "assigned_garage");
        let odometer = extract_f32(block, "odometer");
        let mileage = extract_f32(block, "mileage");

        trucks.push(ParsedTruck {
            truck_id,
            brand,
            model,
            odometer,
            mileage,
            assigned_garage: garage,
        });
    }

    log!("Parsing abgeschlossen. Insgesamt {} Trucks.", trucks.len());

    // Alle Truck-IDs ausgeben
    for truck in &trucks {
        log!(
            "Gefundene Truck-ID: {} | Brand: {} | Model: {}",
            truck.truck_id,
            truck.brand,
            truck.model
        );
    }

    trucks
}

fn extract_value(block: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"{}\s*:\s*"([^"]*)""#, key)).unwrap();
    re.captures(block)
        .and_then(|c| Some(c.get(1).unwrap().as_str().to_string()))
}

fn extract_f32(block: &str, key: &str) -> Option<f32> {
    let re = Regex::new(&format!(r#"{}\s*:\s*([0-9\.\-]+)"#, key)).unwrap();
    re.captures(block)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f32>().ok())
}
