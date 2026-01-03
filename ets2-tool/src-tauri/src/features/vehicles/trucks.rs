use crate::dev_log;
use crate::models::trucks::ParsedTruck;
use crate::shared::sii_parser::{parse_trucks_from_sii, get_player_id, get_vehicle_ids};
use crate::state::AppProfileState;
use super::load_save_content;
use tauri::command;

#[command]
pub async fn get_all_trucks(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>
) -> Result<Vec<ParsedTruck>, String> {
    dev_log!("get_all_trucks: Profil {}", profile_path);

    let content = load_save_content(profile_state)?;

    let trucks = parse_trucks_from_sii(&content);
    dev_log!("{} Trucks gefunden", trucks.len());

    Ok(trucks)
}

#[command]
pub async fn get_player_truck(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>
) -> Result<ParsedTruck, String> {
    dev_log!("get_player_truck: Profil {}", profile_path);

    let content = load_save_content(profile_state)?;

    let trucks = parse_trucks_from_sii(&content);

    let player_id = get_player_id(&content).ok_or("Player ID nicht im economy block gefunden".to_string())?;
    let (player_truck_id_opt, _) = get_vehicle_ids(&content, &player_id);

    let player_truck_id = player_truck_id_opt.ok_or("my_truck nicht im player block gefunden".to_string())?;

    let id_clean = player_truck_id.trim().to_lowercase();

    let base_truck = trucks
        .into_iter()
        .find(|t| t.truck_id.to_lowercase() == id_clean)
        .ok_or(format!("Player Truck mit ID {} nicht gefunden", id_clean))?;

    Ok(base_truck)
}
