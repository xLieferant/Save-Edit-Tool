use crate::dev_log;
use crate::models::trailers::{ParsedTrailer, TrailerData, TrailerDefData};
use crate::models::trucks::ParsedTruck;
use crate::state::AppProfileState;
use crate::shared::sii_parser::{parse_trailers_from_sii, parse_trailer_defs_from_sii, parse_trucks_from_sii, get_player_id, get_vehicle_ids};
use super::load_save_content;
use tauri::command;

#[command]
pub async fn get_player_trailer(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>
) -> Result<Option<ParsedTrailer>, String> {  // ← CHANGED: Now returns Option<ParsedTrailer>
    dev_log!("get_player_trailer: Profil {}", profile_path);

    let content = load_save_content(profile_state)?;

    let trailers_data = parse_trailers_from_sii(&content);
    let defs_data = parse_trailer_defs_from_sii(&content);
    let trucks_data = parse_trucks_from_sii(&content);

    let player_id = get_player_id(&content).ok_or("Player ID nicht im economy block gefunden"
        .to_string())?;
    let (player_truck_id_opt, player_trailer_id_opt) = get_vehicle_ids(
        &content, &player_id);

    // ← CHANGED: Instead of erroring, return None if player has no trailer
    let trailer_id = match player_trailer_id_opt {
        Some(id) => id,
        None => {
            dev_log!("Player has no trailer attached - this is normal");
            return Ok(None);  // ← Return None instead of error
        }
    };

    let id_clean = trailer_id.trim().to_lowercase();

    let trailer_data = trailers_data
        .into_iter()
        .find(|t| t.trailer_id.to_lowercase() == id_clean)
        .ok_or(format!("Player Trailer mit ID {} nicht gefunden", id_clean))?;

    let parsed_trailer = parsed_trailer_from_data(&trailer_data, &defs_data);

    if let Some(player_truck_id) = player_truck_id_opt {
        let truck_id_clean = player_truck_id.trim().to_lowercase();
        if let Some(player_truck) = trucks_data
            .into_iter()
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
        parsed_trailer.odometer,
        parsed_trailer.cargo_mass,
        parsed_trailer.cargo_damage,
        parsed_trailer.body_wear,
        parsed_trailer.chassis_wear,
        &parsed_trailer.wheels_wear
    );

    Ok(Some(parsed_trailer))  // ← CHANGED: Return Some(trailer)
}

#[command]
pub async fn get_all_trailers(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>
) -> Result<Vec<ParsedTrailer>, String> {
    dev_log!("get_all_trailers: Profil {}", profile_path);
    
    let content = load_save_content(profile_state)?;

    let trailers_data = parse_trailers_from_sii(&content);
    let defs_data = parse_trailer_defs_from_sii(&content);

    let parsed_trailers: Vec<ParsedTrailer> = trailers_data
        .into_iter()
        .map(|trailer_data| parsed_trailer_from_data(&trailer_data, &defs_data))
        .collect();

    dev_log!("{} Trailer gefunden", parsed_trailers.len());
    Ok(parsed_trailers)
}

// Hilfsfunktion: ParsedTrailer aus TrailerData
fn parsed_trailer_from_data(tr: &TrailerData, defs: &std::collections::HashMap<String,
    TrailerDefData>) -> ParsedTrailer {
    // Alle Floats über parse_value_auto (Hex oder Float)
    // odometer (f32) + odometer_float (Option<f32>)
    let odometer = tr.odometer + tr.odometer_float.unwrap_or(0.0);

    // In your sii_parser, 'wear_float' corresponds to 'trailer_body_wear'
    let body_wear = tr.wear_float.unwrap_or(0.0);

    // In your sii_parser, 'wheels_float' is Option<Vec<f32>>
    let wheels_wear = tr.wheels_float.clone().unwrap_or_default();

    // Definition Lookup
    let def = defs.get(&tr.trailer_definition).cloned().unwrap_or_default();

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
        
        // Def Data
        gross_trailer_weight_limit: def.gross_trailer_weight_limit,
        chassis_mass: def.chassis_mass,
        body_mass: def.body_mass,
        body_type: def.body_type,
        chain_type: def.chain_type,
        length: def.length,
    }
}