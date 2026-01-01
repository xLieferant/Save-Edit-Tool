use crate::dev_log;
use crate::models::trailers::ParsedTrailer;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::hex_float::parse_value_auto;
use crate::shared::regex_helper::cragex;
use crate::shared::sii_parser::TrailerData;
use crate::shared::sii_parser::{extract_string, extract_string_array, parse_trailers_from_sii};
use std::path::Path;
use tauri::command;

#[command]
pub async fn get_player_trailer(profile_path: String) -> Result<ParsedTrailer, String> {
    dev_log!("get_player_trailer: Profil {}", profile_path);

    let path = format!("{}/save/quicksave/game.sii", profile_path);
    let content = decrypt_if_needed(Path::new(&path)).map_err(|e| {
        dev_log!("Decrypt Fehler: {}", e);
        e
    })?;

    let trailers_data = parse_trailers_from_sii(&content);

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

    Ok(parsed_trailer_from_data(&trailer_data))
}

#[command]
pub async fn get_all_trailers(profile_path: String) -> Result<Vec<ParsedTrailer>, String> {
    dev_log!("get_all_trailers: Profil {}", profile_path);

    let path = format!("{}/save/quicksave/game.sii", profile_path);
    let content = decrypt_if_needed(Path::new(&path)).map_err(|e| {
        dev_log!("Decrypt Fehler: {}", e);
        e
    })?;

    let trailers_data = parse_trailers_from_sii(&content);

    let parsed_trailers: Vec<ParsedTrailer> = trailers_data
        .into_iter()
        .map(|trailer_data| parsed_trailer_from_data(&trailer_data))
        .collect();

    dev_log!("{} Trailer gefunden", parsed_trailers.len());
    Ok(parsed_trailers)
}

// Hilfsfunktion: ParsedTrailer aus TrailerData
fn parsed_trailer_from_data(tr: &TrailerData) -> ParsedTrailer {
    // Alle Floats über parse_value_auto (Hex oder Float)
    // odometer (f32) + odometer_float (Option<f32>)
    let odometer = tr.odometer + tr.odometer_float.unwrap_or(0.0);

    // In your sii_parser, 'wear_float' corresponds to 'trailer_body_wear'
    let body_wear = tr.wear_float.unwrap_or(0.0);

    // In your sii_parser, 'wheels_float' is Option<Vec<f32>>
    let wheels_wear = tr.wheels_float.clone().unwrap_or_default();

    ParsedTrailer {
        trailer_id: tr.trailer_id.clone(),

        // These fields are expected by ParsedTrailer but not present in TrailerData from your parser
        cargo_mass: 0.0,
        cargo_damage: 0.0,
        body_wear_unfixable: 0.0,
        chassis_wear: 0.0,
        chassis_wear_unfixable: 0.0,
        wheels_wear_unfixable: vec![],
        integrity_odometer: 0.0, // Set to default f32 value
        accessories: vec![],     // Set to default empty Vec<String>

        body_wear,
        wheels_wear,
        odometer,
        license_plate: tr.license_plate.clone(),
    }
}
