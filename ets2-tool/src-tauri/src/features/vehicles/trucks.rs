use super::{load_save_content_from_save_path, resolve_active_save_from_snapshot};
use crate::dev_log;
use crate::models::trucks::ParsedTruck;
use crate::shared::paths::game_sii_from_save;
use crate::shared::sii_parser::{get_player_id, get_vehicle_ids, parse_trucks_from_sii};
use crate::shared::trace::TraceScope;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use tauri::command;

fn player_truck_from_content(content: &str, trucks: &[ParsedTruck]) -> Option<ParsedTruck> {
    let player_id = get_player_id(content)?;
    let (player_truck_id_opt, _) = get_vehicle_ids(content, &player_id);
    let player_truck_id = player_truck_id_opt?;
    let id_clean = player_truck_id.trim().to_lowercase();
    trucks
        .iter()
        .find(|truck| truck.truck_id.to_lowercase() == id_clean)
        .cloned()
}

#[command]
pub async fn get_all_trucks(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>,
    profile_cache: tauri::State<'_, ProfileCache>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
) -> Result<Vec<ParsedTruck>, String> {
    let mut trace = TraceScope::new("get_all_trucks");
    dev_log!("get_all_trucks: Profil {}", profile_path);

    let save_path = resolve_active_save_from_snapshot(
        profile_state.current_save.lock().unwrap().clone(),
        profile_state.current_profile.lock().unwrap().clone(),
    )?;
    let path_key = game_sii_from_save(std::path::Path::new(&save_path))
        .display()
        .to_string();

    if let Some(cached) = profile_cache.get_cached_trucks(&path_key) {
        dev_log!("get_all_trucks liefert Cache: {} Eintraege", cached.len());
        trace.finish_ok();
        return Ok(cached);
    }

    let decrypt_cache_cloned = decrypt_cache.inner().clone();
    let (trucks, player_truck) = tauri::async_runtime::spawn_blocking(move || {
        let (content, _) = load_save_content_from_save_path(&save_path, &decrypt_cache_cloned)?;
        let trucks = parse_trucks_from_sii(&content);
        let player_truck = player_truck_from_content(&content, &trucks);
        Ok::<(Vec<ParsedTruck>, Option<ParsedTruck>), String>((trucks, player_truck))
    })
    .await
    .map_err(|error| format!("get_all_trucks join failed: {}", error))??;

    profile_cache.cache_trucks(path_key.clone(), trucks.clone(), player_truck.clone());
    dev_log!("{} Trucks gefunden", trucks.len());
    trace.finish_ok();
    Ok(trucks)
}

#[command]
pub async fn get_player_truck(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>,
    profile_cache: tauri::State<'_, ProfileCache>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
) -> Result<ParsedTruck, String> {
    let mut trace = TraceScope::new("get_player_truck");
    dev_log!("get_player_truck: Profil {}", profile_path);

    let save_path = resolve_active_save_from_snapshot(
        profile_state.current_save.lock().unwrap().clone(),
        profile_state.current_profile.lock().unwrap().clone(),
    )?;
    let path_key = game_sii_from_save(std::path::Path::new(&save_path))
        .display()
        .to_string();

    if let Some(cached) = profile_cache.get_cached_player_truck(&path_key) {
        dev_log!("get_player_truck liefert Cache");
        trace.finish_ok();
        return Ok(cached);
    }

    let decrypt_cache_cloned = decrypt_cache.inner().clone();
    let (trucks, base_truck) = tauri::async_runtime::spawn_blocking(move || {
        let (content, _) = load_save_content_from_save_path(&save_path, &decrypt_cache_cloned)?;
        let trucks = parse_trucks_from_sii(&content);
        let player_id = get_player_id(&content)
            .ok_or("Player ID nicht im economy block gefunden".to_string())?;
        let (player_truck_id_opt, _) = get_vehicle_ids(&content, &player_id);
        let player_truck_id =
            player_truck_id_opt.ok_or("my_truck nicht im player block gefunden".to_string())?;
        let id_clean = player_truck_id.trim().to_lowercase();
        let base_truck = trucks
            .iter()
            .find(|truck| truck.truck_id.to_lowercase() == id_clean)
            .cloned()
            .ok_or(format!("Player Truck mit ID {} nicht gefunden", id_clean))?;
        Ok::<(Vec<ParsedTruck>, ParsedTruck), String>((trucks, base_truck))
    })
    .await
    .map_err(|error| format!("get_player_truck join failed: {}", error))??;

    profile_cache.cache_trucks(path_key.clone(), trucks, Some(base_truck.clone()));
    trace.finish_ok();
    Ok(base_truck)
}

#[cfg(test)]
mod tests {
    use super::player_truck_from_content;
    use crate::shared::sii_parser::{get_vehicle_ids, parse_trucks_from_sii};

    #[test]
    fn vehicle_ids_returns_truck_then_trailer() {
        let content = r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 my_truck: _nameless.truck.active
 my_trailer: _nameless.trailer.attached
}
}
"#;
        let ids = get_vehicle_ids(content, "_nameless.player");
        assert_eq!(ids.0.as_deref(), Some("_nameless.truck.active"));
        assert_eq!(ids.1.as_deref(), Some("_nameless.trailer.attached"));
    }

    #[test]
    fn get_all_trucks_player_resolution_uses_truck_id_not_trailer_id() {
        let content = r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 my_truck: _nameless.truck.active
 my_trailer: _nameless.truck.trailer_like
}
vehicle : _nameless.truck.active {
 accessories: 1
 accessories[0]: _nameless.acc.active
}
vehicle_accessory : _nameless.acc.active {
 data_path: "/def/vehicle/truck/scania.s_2016/data.sii"
}
vehicle : _nameless.truck.trailer_like {
 accessories: 1
 accessories[0]: _nameless.acc.trailer_like
}
vehicle_accessory : _nameless.acc.trailer_like {
 data_path: "/def/vehicle/truck/man.tgx/data.sii"
}
}
"#;
        let trucks = parse_trucks_from_sii(content);
        let player = player_truck_from_content(content, &trucks).unwrap();
        assert_eq!(player.truck_id, "_nameless.truck.active");
    }
}
