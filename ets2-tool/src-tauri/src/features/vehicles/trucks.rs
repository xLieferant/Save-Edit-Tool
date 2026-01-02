use crate::dev_log;
use crate::models::trucks::ParsedTruck;
use crate::shared::regex_helper::cragex;
use crate::shared::sii_parser::parse_trucks_from_sii;
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

    let re_player_truck =
        cragex(r"player\s*:\s*[A-Za-z0-9._]+\s*\{[^}]*?my_truck\s*:\s*([A-Za-z0-9._]+)")
            .map_err(|e| format!("Regex Fehler: {}", e))?;

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

    Ok(base_truck)
}
