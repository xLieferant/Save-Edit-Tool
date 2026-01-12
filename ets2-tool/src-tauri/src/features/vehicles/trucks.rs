use crate::dev_log;
use crate::models::trucks::ParsedTruck;
use crate::shared::sii_parser::{parse_trucks_from_sii, get_player_id, get_vehicle_ids};
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use super::load_save_content;
use tauri::command;

#[command]
pub async fn get_all_trucks(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>,
    profile_cache: tauri::State<'_, ProfileCache>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
) -> Result<Vec<ParsedTruck>, String> {
    dev_log!("get_all_trucks: Profil {}", profile_path);

    let (content, path_key) = load_save_content(profile_state, decrypt_cache)?;

    if let Some(cached) = profile_cache.get_cached_trucks(&path_key) {
        dev_log!("get_all_trucks liefert Cache: {} Einträge", cached.len());
        return Ok(cached);
    }

    let trucks = parse_trucks_from_sii(&content);
    let player_id = get_player_id(&content);
    let player_truck = player_id
        .as_ref()
        .and_then(|id| {
            let (_, player_truck_id_opt) = get_vehicle_ids(&content, id);
            player_truck_id_opt.and_then(|player_truck_id| {
                let id_clean = player_truck_id.trim().to_lowercase();
                trucks
                    .iter()
                    .find(|t| t.truck_id.to_lowercase() == id_clean)
                    .cloned()
            })
        });

    profile_cache.cache_trucks(path_key.clone(), trucks.clone(), player_truck.clone());
    dev_log!("{} Trucks gefunden", trucks.len());

    Ok(trucks)
}

#[command]
pub async fn get_player_truck(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>,
    profile_cache: tauri::State<'_, ProfileCache>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
) -> Result<ParsedTruck, String> {
    dev_log!("get_player_truck: Profil {}", profile_path);

    let (content, path_key) = load_save_content(profile_state, decrypt_cache)?;

    if let Some(cached) = profile_cache.get_cached_player_truck(&path_key) {
        dev_log!("get_player_truck liefert Cache");
        return Ok(cached);
    }

    let trucks = parse_trucks_from_sii(&content);

    let player_id = get_player_id(&content).ok_or("Player ID nicht im economy block gefunden".to_string())?;
    let (player_truck_id_opt, _) = get_vehicle_ids(&content, &player_id);

    let player_truck_id = player_truck_id_opt.ok_or("my_truck nicht im player block gefunden".to_string())?;

    let id_clean = player_truck_id.trim().to_lowercase();

    let base_truck = trucks
        .iter()
        .find(|t| t.truck_id.to_lowercase() == id_clean)
        .cloned()
        .ok_or(format!("Player Truck mit ID {} nicht gefunden", id_clean))?;

    profile_cache.cache_trucks(path_key.clone(), trucks, Some(base_truck.clone()));

    Ok(base_truck)
}
