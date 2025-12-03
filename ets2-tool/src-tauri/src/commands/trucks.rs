use std::path::Path;
use tauri::command;
use crate::log;

use crate::utils::sii_parser::parse_trucks_from_sii;
use crate::models::trucks::ParsedTruck;
// Passe diesen Pfad an, wo auch immer deine Entschlüsselungsfunktion liegt
use crate::utils::decrypt::decrypt_if_needed; 


#[command]
pub async fn get_all_trucks(profile_path: String) -> Result<Vec<ParsedTruck>, String> {
    
    let game_sii_path_str = format!("{}/save/quicksave/game.sii", profile_path);
    let game_sii_path = Path::new(&game_sii_path_str);
    
    log!("Versuche game.sii zu laden/entschlüsseln: {}", game_sii_path.display());

    // Rufe die Entschlüsselungsfunktion auf, sie gibt den String-Inhalt zurück
    let content = decrypt_if_needed(game_sii_path)?;
    
    log!("Inhalt erfolgreich aus game.sii extrahiert und entschlüsselt.");

    // Den lesbaren Inhalt an die Parsing-Logik übergeben
    let trucks = parse_trucks_from_sii(&content);
    
    log!("parse_trucks_from_sii Erfolgreich ausgeführt. Gefundene Trucks: {}", trucks.len());

    Ok(trucks) // Wichtig: Rückgabe in Ok() einpacken
}
