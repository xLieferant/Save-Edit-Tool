use std::path::Path;
use tauri::command;
use crate::log;
use crate::models::trucks::ParsedTruck;
use crate::utils::sii_parser::parse_trucks_from_sii;
use crate::utils::decrypt::decrypt_if_needed; // <-- vergessen


#[command]
pub async fn get_player_truck_info(profile_path: String, player_truck_id: String) -> Result<ParsedTruck, String> {
    let game_sii_path_str = format!("{}/save/quicksave/game.sii", profile_path);
    let game_sii_path = Path::new(&game_sii_path_str);
    log!("Versuche game.sii zu laden/entschlüsseln: {}", game_sii_path.display());

    let content = decrypt_if_needed(game_sii_path)?;
    log!("Inhalt erfolgreich aus game.sii extrahiert und entschlüsselt.");

    let trucks = parse_trucks_from_sii(&content);
    log!("let trucks = parse_truck_from_sii wird ausgeführt! ----------------");
    let truck = trucks.into_iter()
        .find(|t| t.truck_id == player_truck_id)
        .ok_or("Player Truck nicht gefunden")?;

    log!(
    "Player ID: {} fährt Truck: {} ({} {})",
    player_truck_id, // falls du die Player-ID separat hast, sonst passt hier der Truck
    truck.truck_id,
    truck.brand,
    truck.model
);


    Ok(truck)
}

