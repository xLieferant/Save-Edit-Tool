use std::fs;
use tauri::command;
use create::log;

use crate::utils::sii_parser::parse_trucks_from_sii;
use crate::models::trucks::ParsedTruck;

#[command]
pub async fn get_all_trucks(profile_path: String) -> Result<Vec<ParsedTruck>, String> {
    let game_sii_path = format!("{}/save/quicksave/game.sii", profile_path);

    let content = fs::read_to_string(&game_sii_path)
        .map_err(|e| format!("Fehler beim Lesen von game.sii: {}", e))?;

    let trucks = parse_trucks_from_sii(&content);

    Ok(trucks)
}
