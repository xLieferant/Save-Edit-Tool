use crate::utils::sii_parser;
use crate::utils::decrypt::decrypt_if_needed;
use crate::models::trucks::ParsedTruck;
use std::path::Path;
use tauri::command;
use crate::log;

#[command]
pub async fn get_all_trucks(profile_path: String) -> Result<Vec<ParsedTruck>, String> {
    let path_string = format!("{}/save/quicksave/game.sii", profile_path);
    let game_sii_path = Path::new(&path_string);

    log!("Versuche game.sii zu laden: {}", game_sii_path.display());

    let content = decrypt_if_needed(game_sii_path)?;
    let trucks = sii_parser::parse_trucks_from_sii(&content);

    log!("parse_trucks_from_sii → {} Trucks gefunden", trucks.len());
    Ok(trucks)
}

#[command]
pub async fn get_player_truck(profile_path: String, player_truck_id: String) -> Result<ParsedTruck, String> {
    let path_string = format!("{}/save/quicksave/game.sii", profile_path);
    let game_sii_path = Path::new(&path_string);

    let content = decrypt_if_needed(game_sii_path)?;
    let trucks = sii_parser::parse_trucks_from_sii(&content);

    let truck = trucks.into_iter()
        .find(|t| t.truck_id == player_truck_id)
        .ok_or("Player Truck nicht gefunden")?;

    log!("Player Truck gefunden: {} ({}, {})", truck.truck_id, truck.brand, truck.model);
    Ok(truck)
}
