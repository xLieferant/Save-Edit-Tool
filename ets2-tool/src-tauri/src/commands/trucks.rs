use std::path::Path;
use tauri::command;
use crate::log;

use crate::utils::sii_parser::parse_trucks_from_sii;
use crate::models::trucks::ParsedTruck;
// Passe diesen Pfad an, wo auch immer deine Entschlüsselungsfunktion liegt
use crate::utils::decrypt::decrypt_if_needed; 


#[command]
pub async fn get_player_truck_info(profile_path: String, player_truck_id: String) -> Result<ParsedTruck, String> {
    let game_sii_path_str = format!("{}/save/quicksave/game.sii", profile_path);
    let game_sii_path = Path::new(&game_sii_path_str);
    log!("Versuche game.sii zu laden/entschlüsseln: {}", game_sii_path.display());

    let content = decrypt_if_needed(game_sii_path)?;
    log!("Inhalt erfolgreich aus game.sii extrahiert und entschlüsselt.");

    let trucks = parse_trucks_from_sii(&content);

    let truck = trucks.into_iter()
        .find(|t| t.truck_id == player_truck_id)
        .ok_or("Player Truck nicht gefunden")?;

    log!("Player Truck gefunden: ID={}, Brand={}, Model={}", truck.truck_id, truck.brand, truck.model);

    Ok(truck)
}