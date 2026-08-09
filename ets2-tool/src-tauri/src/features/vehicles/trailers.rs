use super::{
    load_save_content, load_save_content_from_save_path, resolve_active_save_from_snapshot,
};
use crate::dev_log;
use crate::features::trailer_change::parser::{
    ParsedTrailerSave, find_trailer_block_by_id, find_unit_block_by_id, parse_trailer_save,
    resolve_current_trailer_pointer,
};
use crate::features::truck_change::parser::{extract_field_value, is_null_ref};
use crate::features::vehicles::license_plate::license_plate_display_text;
use crate::models::trailers::{ParsedTrailer, PlayerTrailerResult, TrailerData, TrailerDefData};
use crate::shared::paths::game_sii_from_save;
use crate::shared::sii_parser::{
    get_player_id, get_vehicle_ids, parse_trailer_defs_from_sii, parse_trailers_from_sii,
    parse_trucks_from_sii,
};
use crate::shared::trace::TraceScope;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use tauri::command;

#[command]
pub async fn get_player_trailer(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>,
    profile_cache: tauri::State<'_, ProfileCache>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
) -> Result<PlayerTrailerResult, String> {
    let mut trace = TraceScope::new("get_player_trailer");
    dev_log!("get_player_trailer: Profil {}", profile_path);

    let save_path = resolve_active_save_from_snapshot(
        profile_state.current_save.lock().unwrap().clone(),
        profile_state.current_profile.lock().unwrap().clone(),
    )?;
    let path_key = game_sii_from_save(std::path::Path::new(&save_path))
        .display()
        .to_string();

    let decrypt_cache_cloned = decrypt_cache.inner().clone();
    let worker_result = tauri::async_runtime::spawn_blocking(move || {
        let (content, _) = load_save_content_from_save_path(&save_path, &decrypt_cache_cloned)?;
        let parsed_context = parse_trailer_save(&content);
        let has_active_job = player_has_active_job(&parsed_context);
        let trailers_data = parse_trailers_from_sii(&content);
        let defs_data = parse_trailer_defs_from_sii(&content);
        let parsed_trailers: Vec<ParsedTrailer> = trailers_data
            .iter()
            .map(|trailer_data| parsed_trailer_from_data(trailer_data, &defs_data))
            .collect();
        let active_job_cargo_mass = active_job_trailer_id(&parsed_context).and_then(|trailer_id| {
            parsed_trailers
                .iter()
                .find(|trailer| trailer.trailer_id.eq_ignore_ascii_case(&trailer_id))
                .map(|trailer| trailer.cargo_mass)
        });
        let trucks_data = parse_trucks_from_sii(&content);
        let player_id = get_player_id(&content)
            .ok_or("Player ID nicht im economy block gefunden".to_string())?;
        let (player_truck_id_opt, _) = get_vehicle_ids(&content, &player_id);
        let player_trailer_id_opt = resolve_current_trailer_pointer(&parsed_context)
            .ok()
            .filter(|pointer| pointer.writable)
            .map(|pointer| pointer.trailer_id);

        let player_trailer = match player_trailer_id_opt {
            Some(id) => {
                let id_clean = id.trim().to_lowercase();
                parsed_trailers
                    .iter()
                    .find(|t| t.trailer_id.to_lowercase() == id_clean)
                    .cloned()
            }
            None => None,
        };

        Ok::<
            (
                Vec<ParsedTrailer>,
                Option<ParsedTrailer>,
                Vec<crate::models::trucks::ParsedTruck>,
                Option<String>,
                bool,
                Option<f32>,
            ),
            String,
        >((
            parsed_trailers,
            player_trailer,
            trucks_data,
            player_truck_id_opt,
            has_active_job,
            active_job_cargo_mass,
        ))
    })
    .await
    .map_err(|error| format!("get_player_trailer join failed: {}", error))??;

    let (
        parsed_trailers,
        player_trailer,
        trucks_data,
        player_truck_id_opt,
        has_active_job,
        active_job_cargo_mass,
    ) = worker_result;
    if player_trailer.is_none() {
        dev_log!("Player has no trailer attached - this is normal");
        profile_cache.cache_trailers(path_key.clone(), parsed_trailers.clone(), None);
        trace.finish_ok();
        return Ok(PlayerTrailerResult::none()
            .with_active_job(has_active_job)
            .with_active_job_cargo_mass(active_job_cargo_mass));
    }

    let player_trailer = player_trailer.unwrap();
    if let Some(player_truck_id) = player_truck_id_opt {
        let truck_id_clean = player_truck_id.trim().to_lowercase();
        if let Some(player_truck) = trucks_data
            .iter()
            .find(|t| t.truck_id.to_lowercase() == truck_id_clean)
        {
            dev_log!(
                "Player Truck Data: Odometer: {}, Fuel: {}, Engine Wear: {}, Transmission Wear: {},\
                Cabin Wear: {}, Chassis Wear: {}, Wheels Wear: {:?}",
                player_truck.odometer,
                player_truck.fuel_relative,
                player_truck.engine_wear,
                player_truck.transmission_wear,
                player_truck.cabin_wear,
                player_truck.chassis_wear,
                &player_truck.wheels_wear
            );
        }
    }

    dev_log!(
        "Player Trailer Data: Odometer: {}, Cargo Mass: {}, Cargo Damage: {}, Body Wear: {},\
        Chassis Wear: {}, Wheels Wear: {:?}",
        player_trailer.odometer,
        player_trailer.cargo_mass,
        player_trailer.cargo_damage,
        player_trailer.body_wear,
        player_trailer.chassis_wear,
        &player_trailer.wheels_wear
    );

    profile_cache.cache_trailers(path_key, parsed_trailers, Some(player_trailer.clone()));
    trace.finish_ok();
    return Ok(PlayerTrailerResult::some(player_trailer)
        .with_active_job(has_active_job)
        .with_active_job_cargo_mass(active_job_cargo_mass));

    let (content, path_key) = load_save_content(profile_state, decrypt_cache)?;

    if let Some(cached) = profile_cache.get_cached_player_trailer(&path_key) {
        dev_log!("get_player_trailer liefert Cache");
        return Ok(player_trailer_result(cached));
    }

    let trailers_data = parse_trailers_from_sii(&content);
    let defs_data = parse_trailer_defs_from_sii(&content);
    let parsed_trailers: Vec<ParsedTrailer> = trailers_data
        .iter()
        .map(|trailer_data| parsed_trailer_from_data(trailer_data, &defs_data))
        .collect();
    let trucks_data = parse_trucks_from_sii(&content);

    let player_id =
        get_player_id(&content).ok_or("Player ID nicht im economy block gefunden".to_string())?;
    let (player_truck_id_opt, player_trailer_id_opt) = get_vehicle_ids(&content, &player_id);

    let player_trailer = match player_trailer_id_opt {
        Some(id) => {
            let id_clean = id.trim().to_lowercase();
            match parsed_trailers
                .iter()
                .find(|t| t.trailer_id.to_lowercase() == id_clean)
                .cloned()
            {
                Some(trailer) => trailer,
                None => {
                    dev_log!(
                        "Player trailer reference {} was not found in parsed trailers",
                        id_clean
                    );
                    profile_cache.cache_trailers(path_key.clone(), parsed_trailers.clone(), None);
                    return Ok(PlayerTrailerResult::none());
                }
            }
        }
        None => {
            dev_log!("Player has no trailer attached - this is normal");
            profile_cache.cache_trailers(path_key.clone(), parsed_trailers.clone(), None);
            return Ok(PlayerTrailerResult::none());
        }
    };

    if let Some(player_truck_id) = player_truck_id_opt {
        let truck_id_clean = player_truck_id.trim().to_lowercase();
        if let Some(player_truck) = trucks_data
            .iter()
            .find(|t| t.truck_id.to_lowercase() == truck_id_clean)
        {
            dev_log!(
                "Player Truck Data: Odometer: {}, Fuel: {}, Engine Wear: {}, Transmission Wear: {},\
                Cabin Wear: {}, Chassis Wear: {}, Wheels Wear: {:?}",
                player_truck.odometer,
                player_truck.fuel_relative,
                player_truck.engine_wear,
                player_truck.transmission_wear,
                player_truck.cabin_wear,
                player_truck.chassis_wear,
                &player_truck.wheels_wear
            );
        }
    }

    dev_log!(
        "Player Trailer Data: Odometer: {}, Cargo Mass: {}, Cargo Damage: {}, Body Wear: {},\
        Chassis Wear: {}, Wheels Wear: {:?}",
        player_trailer.odometer,
        player_trailer.cargo_mass,
        player_trailer.cargo_damage,
        player_trailer.body_wear,
        player_trailer.chassis_wear,
        &player_trailer.wheels_wear
    );

    profile_cache.cache_trailers(path_key, parsed_trailers, Some(player_trailer.clone()));
    Ok(PlayerTrailerResult::some(player_trailer))
}

fn player_trailer_result(trailer: Option<ParsedTrailer>) -> PlayerTrailerResult {
    match trailer {
        Some(trailer) => PlayerTrailerResult::some(trailer),
        None => PlayerTrailerResult::none(),
    }
}

fn player_has_active_job(parsed: &ParsedTrailerSave) -> bool {
    let Some(player_id) = parsed.player_id.as_deref() else {
        return false;
    };
    let Some(player_block) = find_unit_block_by_id(&parsed.unit_blocks, player_id, Some("player"))
    else {
        return false;
    };
    let Some(job_id) = extract_field_value(&player_block.raw_block, "current_job") else {
        return false;
    };

    !is_null_ref(&job_id) && find_unit_block_by_id(&parsed.unit_blocks, &job_id, None).is_some()
}

fn active_job_trailer_id(parsed: &ParsedTrailerSave) -> Option<String> {
    let player_id = parsed.player_id.as_deref()?;
    let player_block = find_unit_block_by_id(&parsed.unit_blocks, player_id, Some("player"))?;
    let job_id = extract_field_value(&player_block.raw_block, "current_job")?;
    if is_null_ref(&job_id) {
        return None;
    }
    let job_block = find_unit_block_by_id(&parsed.unit_blocks, &job_id, None)?;

    if let Some(company_trailer_id) = extract_field_value(&job_block.raw_block, "company_trailer")
        .filter(|value| !is_null_ref(value))
    {
        return find_trailer_block_by_id(&parsed.trailer_blocks, &company_trailer_id)
            .map(|block| block.id.clone());
    }

    resolve_current_trailer_pointer(parsed)
        .ok()
        .filter(|pointer| pointer.writable)
        .map(|pointer| pointer.trailer_id)
}

#[command]
pub async fn get_all_trailers(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>,
    profile_cache: tauri::State<'_, ProfileCache>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
) -> Result<Vec<ParsedTrailer>, String> {
    let mut trace = TraceScope::new("get_all_trailers");
    dev_log!("get_all_trailers: Profil {}", profile_path);

    let save_path = resolve_active_save_from_snapshot(
        profile_state.current_save.lock().unwrap().clone(),
        profile_state.current_profile.lock().unwrap().clone(),
    )?;
    let path_key = game_sii_from_save(std::path::Path::new(&save_path))
        .display()
        .to_string();

    if let Some(cached) = profile_cache.get_cached_trailers(&path_key) {
        dev_log!("get_all_trailers liefert Cache");
        trace.finish_ok();
        return Ok(cached);
    }

    let decrypt_cache_cloned = decrypt_cache.inner().clone();
    let parsed_trailers = tauri::async_runtime::spawn_blocking(move || {
        let (content, _) = load_save_content_from_save_path(&save_path, &decrypt_cache_cloned)?;
        let trailers_data = parse_trailers_from_sii(&content);
        let defs_data = parse_trailer_defs_from_sii(&content);
        Ok::<Vec<ParsedTrailer>, String>(
            trailers_data
                .iter()
                .map(|trailer_data| parsed_trailer_from_data(trailer_data, &defs_data))
                .collect(),
        )
    })
    .await
    .map_err(|error| format!("get_all_trailers join failed: {}", error))??;

    let player_trailer = profile_cache.get_cached_player_trailer(&path_key).flatten();
    profile_cache.cache_trailers(path_key.clone(), parsed_trailers.clone(), player_trailer);

    dev_log!("{} Trailer gefunden", parsed_trailers.len());
    trace.finish_ok();
    return Ok(parsed_trailers);

    let (content, path_key) = load_save_content(profile_state, decrypt_cache)?;

    if let Some(cached) = profile_cache.get_cached_trailers(&path_key) {
        dev_log!("get_all_trailers liefert Cache");
        return Ok(cached);
    }

    let trailers_data = parse_trailers_from_sii(&content);
    let defs_data = parse_trailer_defs_from_sii(&content);

    let parsed_trailers: Vec<ParsedTrailer> = trailers_data
        .iter()
        .map(|trailer_data| parsed_trailer_from_data(trailer_data, &defs_data))
        .collect();

    let player_trailer = profile_cache.get_cached_player_trailer(&path_key).flatten();

    profile_cache.cache_trailers(path_key.clone(), parsed_trailers.clone(), player_trailer);

    dev_log!("{} Trailer gefunden", parsed_trailers.len());
    Ok(parsed_trailers)
}

// Hilfsfunktion: ParsedTrailer aus TrailerData
fn parsed_trailer_from_data(
    tr: &TrailerData,
    defs: &std::collections::HashMap<String, TrailerDefData>,
) -> ParsedTrailer {
    let odometer = tr.odometer + tr.odometer_float.unwrap_or(0.0);
    let body_wear = tr.wear_float.unwrap_or(0.0);
    let wheels_wear = tr.wheels_float.clone().unwrap_or_default();
    let def = defs
        .get(&tr.trailer_definition)
        .cloned()
        .unwrap_or_default();
    let display_license_plate = tr
        .license_plate
        .as_deref()
        .map(license_plate_display_text)
        .filter(|plate| !plate.is_empty());

    ParsedTrailer {
        trailer_id: tr.trailer_id.clone(),
        trailer_definition: tr.trailer_definition.clone(),

        cargo_mass: tr.cargo_mass,
        cargo_damage: tr.cargo_damage,

        body_wear_unfixable: tr.body_wear_unfixable,
        chassis_wear: tr.chassis_wear,
        chassis_wear_unfixable: tr.chassis_wear_unfixable,
        wheels_wear_unfixable: tr.wheels_wear_unfixable.clone(),

        integrity_odometer: tr.integrity_odometer,
        accessories: tr.accessories.clone(),

        body_wear,
        wheels_wear,
        odometer,
        license_plate: tr.license_plate.clone(),
        display_license_plate,

        gross_trailer_weight_limit: def.gross_trailer_weight_limit,
        chassis_mass: def.chassis_mass,
        body_mass: def.body_mass,
        body_type: def.body_type,
        chain_type: def.chain_type,
        length: def.length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job_fixture(company_trailer: &str) -> String {
        format!(
            r#"SiiNunit
{{
economy : _nameless.economy {{
 player: _nameless.player
}}
player : _nameless.player {{
 assigned_vehicles: _nameless.assigned.1
 current_job: _nameless.job
 trailers: 2
 trailers[0]: _nameless.trailer.active
 trailers[1]: _nameless.trailer.company
}}
player_vehicles : _nameless.assigned.1 {{
 vehicle: _nameless.truck.active
 trailer: _nameless.trailer.active
}}
player_job : _nameless.job {{
 company_trailer: {company_trailer}
}}
trailer : _nameless.trailer.active {{
 cargo_mass: &3f800000
}}
trailer : _nameless.trailer.company {{
 cargo_mass: &40000000
}}
}}
"#
        )
    }

    #[test]
    fn resolves_company_trailer_for_job_weight_context() {
        let parsed = parse_trailer_save(&job_fixture("_nameless.trailer.company"));
        assert_eq!(
            active_job_trailer_id(&parsed).as_deref(),
            Some("_nameless.trailer.company")
        );
    }

    #[test]
    fn resolves_active_owned_trailer_for_job_weight_context() {
        let parsed = parse_trailer_save(&job_fixture("null"));
        assert_eq!(
            active_job_trailer_id(&parsed).as_deref(),
            Some("_nameless.trailer.active")
        );
    }
}
