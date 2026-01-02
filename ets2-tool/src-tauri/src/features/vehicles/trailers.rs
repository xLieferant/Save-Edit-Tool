use crate::dev_log;
use crate::models::trailers::{ParsedTrailer, TrailerData, TrailerDefData};
use crate::state::AppProfileState;
use crate::shared::regex_helper::cragex;
use crate::shared::sii_parser::{parse_trailers_from_sii, parse_trailer_defs_from_sii};
use super::load_save_content;
use tauri::command;

#[command]
pub async fn get_player_trailer(
    profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>
) -> Result<ParsedTrailer, String> {
    dev_log!("get_player_trailer: Profil {}", profile_path);

    let content = load_save_content(profile_state)?;

    let trailers_data = parse_trailers_from_sii(&content);
    let defs_data = parse_trailer_defs_from_sii(&content);

    let re_player_trailer =
        cragex(r"player\s*:\s*[A-Za-z0-9._]+\s*\{[^}]*?my_trailer\s*:\s*([A-Za-z0-9._]+)")
            .map_err(|e| format!("Regex Fehler: {}", e))?;

    let trailer_id = re_player_trailer
        .captures(&content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or("my_trailer nicht gefunden".to_string())?;

    let id_clean = trailer_id.trim().to_lowercase();

    let trailer_data = trailers_data
        .into_iter()
        .find(|t| t.trailer_id.to_lowercase() == id_clean)
        .ok_or("Player Trailer nicht gefunden".to_string())?;

    Ok(parsed_trailer_from_data(&trailer_data, &defs_data))
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
fn parsed_trailer_from_data(tr: &TrailerData, defs: &std::collections::HashMap<String, TrailerDefData>) -> ParsedTrailer {
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
