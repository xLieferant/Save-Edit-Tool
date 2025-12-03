use regex::Regex;
use crate::models::trucks::ParsedTruck;
use crate::log;

pub fn parse_trucks_from_sii(content: &str) -> Vec<ParsedTruck> {
    let mut trucks = Vec::new();

     log!("Starte Truck-Parsing");

    // Fahrzeug-Blöcke erkennen
    let re_block = Regex::new(r"vehicle\s*:\s*([^\s]+)\s*\{([^}]+)\}").unwrap();

    for caps in re_block.captures_iter(content) {
        let truck_id = caps.get(1).unwrap().as_str().trim().to_string();
        let block = caps.get(2).unwrap().as_str();

        let brand = extract_value(block, "brand").unwrap_or_default();
        let model = extract_value(block, "model").unwrap_or_default();
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
