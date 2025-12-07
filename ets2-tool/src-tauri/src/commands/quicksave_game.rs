use crate::log;
use crate::models::quicksave_game_info::GameDataQuicksave;
use crate::utils::regex_helper::cragex;
use crate::utils::sii_parser::parse_trucks_from_sii;
use crate::utils::paths::quicksave_game_path;
use crate::utils::decrypt::decrypt_if_needed;
use std::env;
use tauri::command;

#[command]
pub async fn quicksave_game_info() -> Result<GameDataQuicksave, String> {
    log!("Lese Quicksave Game.sii");

    // -------------------------------
    // Profil laden
    // -------------------------------
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;
    let path = quicksave_game_path(&profile);
    let content = decrypt_if_needed(&path)?;

    // Player Block
    let re_player_full = cragex(r"player\s*:\s*([a-zA-Z0-9._]+)\s*\{([^}]*)\}")?;
    let player_caps = re_player_full
        .captures(&content)
        .ok_or("Player Block nicht gefunden")?;

    let player_id = player_caps.get(1).unwrap().as_str().trim().to_string();
    let player_block = player_caps.get(2).unwrap().as_str();

    let player_my_truck = cragex(r"my_truck\s*:\s*([a-zA-Z0-9._]+|null)")?
        .captures(player_block)
        .map(|c| c[1].to_string())
        .filter(|v| v != "null");

    let player_my_trailer = cragex(r"my_trailer\s*:\s*([a-zA-Z0-9._]+|null)")?
        .captures(player_block)
        .map(|c| c[1].to_string())
        .filter(|v| v != "null");

    let player_xp = cragex(r"experience_points:\s*(\d+)")?
        .captures(&content)
        .and_then(|c| c[1].parse::<i64>().ok());

    log!("Player ID: {}", player_id);
    log!("My Truck: {:?}", player_my_truck);

    // -------------------------------
    // Bank block
    // -------------------------------
    let re_bank_full = cragex(r"bank\s*:\s*([a-zA-Z0-9._]+)\s*\{([^}]*)\}")?;
    let bank_caps = re_bank_full
        .captures(&content)
        .ok_or("Bank Block nicht gefunden")?;
    let bank_id = bank_caps.get(1).unwrap().as_str().trim().to_string();
    let bank_block = bank_caps.get(2).unwrap().as_str();

    let player_money = cragex(r"money_account:\s*(\d+)")?
        .captures(bank_block)
        .map(|c| c[1].to_string())
        .filter(|v| v != "null");

    log!("Bank ID: {}", bank_id);
    log!("Player Money: {:?}", player_money);

    // -------------------------------
    // Skills
    // -------------------------------
    let parse_skill = |name: &str| -> Option<i64> {
        cragex(&format!(r"{}:\s*(\d+)", name))
            .ok()?
            .captures(&content)
            .and_then(|c| c[1].parse().ok())
    };

    let adr = parse_skill("adr");
    let long_dist = parse_skill("long_dist");
    let heavy = parse_skill("heavy");
    let fragile = parse_skill("fragile");
    let urgent = parse_skill("urgent");
    let mechanical = parse_skill("mechanical");

    // -------------------------------
    // Vehicle Block + Player Truck Info
    // -------------------------------
    let truck_id = player_my_truck.clone().ok_or("Kein my_truck im Player gefunden")?;
    let vehicle_regex = format!(r"vehicle\s*:\s*{}\s*\{{([^}}]+)}}", regex::escape(&truck_id));
    let vehicle_block = cragex(&vehicle_regex)?
        .captures(&content)
        .ok_or("Vehicle Block nicht gefunden")?
        .get(1)
        .unwrap()
        .as_str();

    let odometer = cragex(r"odometer:\s*(\d+)")?
        .captures(vehicle_block)
        .and_then(|c| c[1].parse().ok());

    let trip_fuel_l = cragex(r"trip_fuel_l:\s*(\d+)")?
        .captures(vehicle_block)
        .and_then(|c| c[1].parse().ok());

    let license_plate = cragex(r#"license_plate:\s*"([^"]+)""#)?
        .captures(vehicle_block)
        .map(|c| c[1].to_string());

    let trucks = parse_trucks_from_sii(&content);
    let player_truck_info = trucks.iter().find(|t| t.truck_id == truck_id);

    if let Some(truck) = player_truck_info {
        log!(
            "Player Truck Info → ID: {}, Brand: {}, Model: {}",
            truck.truck_id,
            truck.brand,
            truck.model
        );
    }

    Ok(GameDataQuicksave {
        player_id: Some(player_id),
        bank_id: Some(bank_id),
        player_xp,
        player_my_truck,
        player_my_trailer,
        adr,
        long_dist,
        heavy,
        fragile,
        urgent,
        mechanical,
        vehicle_id: Some(truck_id),
        brand_path: player_truck_info.map(|t| t.brand.clone()),
        license_plate,
        odometer,
        trip_fuel_l,
    })
}
