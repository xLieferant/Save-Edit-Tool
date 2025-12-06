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

    let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;

    let path = quicksave_game_path(&profile);
    let content = decrypt_if_needed(&path)?;

    // ------------------------------------------------------------
    // 1. Profil lesen
    // ------------------------------------------------------------

    // Player-ID
    let re_player = cragex(r"player:\s*([a-zA-Z0-9._]+)")?;
    log!("re_player befehl durchgelaufen!");

    // Player-Block
    let re_player_full = cragex(r"player\s*:\s*([a-zA-Z0-9._]+)\s*\{([^}]*)\}")?;

    let player_caps = re_player_full
        .captures(&content)
        .ok_or("Player Block nicht gefunden")?;

    // ID extrahieren
    let player_id_string = player_caps
        .get(1)
        .ok_or("Player ID Parsing Error")?
        .as_str()
        .trim()
        .to_string();

    // Block extrahieren
    let player_block = player_caps
        .get(2)
        .ok_or("Player Block Parsing Error")?
        .as_str();

    log!("player_id gefunden! player: {}", player_id_string);
    log!("player_block information: {}", player_block);

    // ------------------------------------------------------------
    // 2. Profil lesen
    // ------------------------------------------------------------

    // Bank-ID
    let re_bank_name = cragex(r"bank:\s*([a-zA-Z0-9._]+)")?;
    log!("re_bank_name befehl durchgelaufen!");

    // Bank-Block
    let re_bank_name_full = cragex(r"bank\s*:\s*([a-zA-Z0-9._]+)\s*\{([^}]*)\}")?;

    let bank_caps = re_bank_name_full
        .captures(&content)
        .ok_or("Bank Block nicht gefunden")?;

    // ID extrahieren
    let bank_id_string = bank_caps
        .get(1)
        .ok_or("Bank Parsing Error")?
        .as_str()
        .trim()
        .to_string();

    // Block extrahieren
    let bank_block = bank_caps
        .get(2)
        .ok_or("Bank Block Parsing Error")?
        .as_str();

    log!("Bank_ID gefunden! bank: {}", bank_id_string);
    log!("bank_block information: {}", bank_block);

    // ------------------------------------------------------------
    // 3. Skills lesen
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
    // 4. Player-Felder extrahieren
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

    let truck_id = my_truck.clone().ok_or("Kein my_truck im Player gefunden")?;

    // ------------------------------------------------------------
    // 5. Vehicle-Block finden
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
    // 6. Werte aus Fahrzeugblock lesen
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
    // 7. Rückgabe
    // ------------------------------------------------------------

    Ok(GameDataQuicksave {
        player_id: Some(player_id_string),
        bank_id: Some(bank_id_string),
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
