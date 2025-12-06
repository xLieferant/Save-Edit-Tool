use crate::log;
use crate::models::quicksave_game_info::GameDataQuicksave;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::quicksave_game_path;
use crate::utils::regex_helper::cragex;
use std::env;
use tauri::command;

#[command]
pub fn quicksave_game_info() -> Result<GameDataQuicksave, String> {
    log!("Lese Quicksave Game.sii");

    let profile = env::var("CURRENT_PROFILE")
        .map_err(|_| "Kein Profil geladen.".to_string())?;

    let path = quicksave_game_path(&profile);
    let content = decrypt_if_needed(&path)?;

    // ------------------------------------------------------------
    // 1. Skills lesen
    // ------------------------------------------------------------

    let adr = cragex(r"adr:\s*(\d+)")
        .and_then(|re| Ok(re.captures(&content)))
        .ok()
        .flatten()
        .and_then(|c| c[1].parse().ok());

    let long_dist = cragex(r"long_dist:\s*(\d+)")
        .and_then(|re| Ok(re.captures(&content)))
        .ok()
        .flatten()
        .and_then(|c| c[1].parse().ok());

    let heavy = cragex(r"heavy:\s*(\d+)")
        .and_then(|re| Ok(re.captures(&content)))
        .ok()
        .flatten()
        .and_then(|c| c[1].parse().ok());

    let fragile = cragex(r"fragile:\s*(\d+)")
        .and_then(|re| Ok(re.captures(&content)))
        .ok()
        .flatten()
        .and_then(|c| c[1].parse().ok());

    let urgent = cragex(r"urgent:\s*(\d+)")
        .and_then(|re| Ok(re.captures(&content)))
        .ok()
        .flatten()
        .and_then(|c| c[1].parse().ok());

    let mechanical = cragex(r"mechanical:\s*(\d+)")
        .and_then(|re| Ok(re.captures(&content)))
        .ok()
        .flatten()
        .and_then(|c| c[1].parse().ok());

    // ------------------------------------------------------------
    // 2. Player finden
    // ------------------------------------------------------------
    let re_player = cragex(r"player\s*:\s*([a-zA-Z0-9._]+)\s*\{([^}]*)\}")?;

    let player_caps = re_player
        .captures(&content)
        .ok_or("Player Block nicht gefunden")?;

    let player_block = player_caps.get(2)
        .ok_or("Player Block Parsing Error")?
        .as_str();

    // ------------------------------------------------------------
    // 3. Player-Felder extrahieren
    // ------------------------------------------------------------

    let my_truck = cragex(r"my_truck:\s*([a-zA-Z0-9._]+|null)")?
        .captures(player_block)
        .map(|c| c[1].to_string())
        .filter(|v| v != "null");

    let my_trailer = cragex(r"my_trailer:\s*([a-zA-Z0-9._]+|null)")?
        .captures(player_block)
        .map(|c| c[1].to_string())
        .filter(|v| v != "null");

    // Truck-Liste lesen
    let mut trucks: Vec<String> = Vec::new();

    let re_trucks = cragex(r"trucks\[\d+\]:\s*([a-zA-Z0-9._]+)")?;
    for cap in re_trucks.captures_iter(player_block) {
        trucks.push(cap[1].to_string());
    }

    let truck_id = my_truck
        .clone()
        .ok_or("Kein my_truck im Player gefunden")?;

    // ------------------------------------------------------------
    // 4. Vehicle-Block finden
    // ------------------------------------------------------------
    let vehicle_regex = format!(
        r"vehicle\s*:\s*{}\s*\{{([^}}]+)}}",
        regex::escape(&truck_id)
    );

    let re_vehicle_block = cragex(&vehicle_regex)?;

    let vehicle_caps = re_vehicle_block
        .captures(&content)
        .ok_or("Vehicle Block des my_truck nicht gefunden")?;

    let vehicle_block = vehicle_caps
        .get(1)
        .ok_or("Vehicle Block fehlerhaft")?
        .as_str();

    // ------------------------------------------------------------
    // 5. Werte aus Fahrzeugblock lesen
    // ------------------------------------------------------------

    let odometer = cragex(r"odometer:\s*(\d+)")?
        .captures(vehicle_block)
        .and_then(|c| c[1].parse().ok());

    let trip_fuel_l = cragex(r"trip_fuel_l:\s*(\d+)")?
        .captures(vehicle_block)
        .and_then(|c| c[1].parse().ok());

    let license_plate = cragex(r#"license_plate:\s*"([^"]+)""#)?
        .captures(vehicle_block)
        .map(|c| c[1].to_string());

    // ------------------------------------------------------------
    // 6. Rückgabe
    // ------------------------------------------------------------

    Ok(GameDataQuicksave {
        adr,
        long_dist,
        heavy,
        fragile,
        urgent,
        mechanical,

        vehicle_id: Some(truck_id),
        brand_path: None,
        license_plate,
        odometer,
        trip_fuel_l,
    })
}
