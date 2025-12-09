use crate::utils::sii_parser::parse_trucks_from_sii;
use crate::utils::decrypt::decrypt_if_needed;
use crate::models::trucks::ParsedTruck;
use crate::utils::regex_helper::cragex;
use crate::log;
use tauri::command;
use std::path::Path;

#[command]
pub async fn get_all_trucks(profile_path: String) -> Result<Vec<ParsedTruck>, String> {
    log!("get_all_trucks: Profil {}", profile_path);

    let path = format!("{}/save/quicksave/game.sii", profile_path);

    let content = decrypt_if_needed(Path::new(&path)).map_err(|e| {
        log!("Decrypt Fehler: {}", e);
        e
    })?;

    let trucks = parse_trucks_from_sii(&content);
    log!("{} Trucks gefunden", trucks.len());

    Ok(trucks)
}

#[command]
pub async fn get_player_truck(profile_path: String) -> Result<ParsedTruck, String> {
    log!("get_player_truck: Profil {}", profile_path);

    let path = format!("{}/save/quicksave/game.sii", profile_path);

    let content = decrypt_if_needed(Path::new(&path)).map_err(|e| {
        log!("Decrypt Fehler: {}", e);
        e
    })?;

    let trucks = parse_trucks_from_sii(&content);

    let re_player_truck = cragex(
        r"player\s*:\s*[A-Za-z0-9._]+\s*\{[^}]*?my_truck\s*:\s*([A-Za-z0-9._]+)"
    ).map_err(|e| format!("Regex Fehler: {}", e))?;

    let player_truck_id = re_player_truck
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|v| v.as_str().to_string())
        .ok_or("my_truck nicht gefunden".to_string())?;

    let id_clean = player_truck_id.trim().to_lowercase();

    let base_truck = trucks
        .into_iter()
        .find(|t| t.truck_id.to_lowercase() == id_clean)
        .ok_or("Player Truck nicht gefunden".to_string())?;

    // vehicle block
    let vehicle_regex = format!(
        r"vehicle\s*:\s*{}\s*\{{([\s\S]*?)\}}",
        regex::escape(&base_truck.truck_id)
    );

    let re_vehicle = cragex(&vehicle_regex).map_err(|e| format!("Regex Fehler Vehicle: {}", e))?;
    let block = re_vehicle
        .captures(&content)
        .and_then(|c| c.get(1).map(|x| x.as_str()))
        .unwrap_or("");

    let extract_string = |pattern: &str| {
        regex::Regex::new(pattern)
            .ok()
            .and_then(|re| re.captures(block))
            .and_then(|c| c.get(1))
            .map(|v| v.as_str().to_string())
    };

    let extract_i64 = |pattern: &str| {
        regex::Regex::new(pattern)
            .ok()
            .and_then(|re| re.captures(block))
            .and_then(|c| c.get(1))
            .and_then(|v| v.as_str().parse::<i64>().ok())
    };

    let odometer = extract_i64(r"odometer:\s*(-?\d+)");
    let trip_fuel_l = extract_i64(r"trip_fuel_l:\s*(-?\d+)");
    let license_plate = extract_string(r#"license_plate:\s*"(.*?)""#);

    Ok(ParsedTruck {
        truck_id: base_truck.truck_id,
        brand: base_truck.brand,
        model: base_truck.model,
        odometer,
        mileage: None,
        trip_fuel_l,
        license_plate,
        assigned_garage: None,
    })
}
