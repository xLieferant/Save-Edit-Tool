use crate::log;
use crate::models::quicksave_game_info::GameDataQuicksave;
use crate::state::{AppProfileState, DecryptCache};
use crate::utils::current_profile::{
    get_current_profile, require_current_profile, require_current_save,
};
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::{game_sii_from_save, quicksave_game_path};
use crate::utils::regex_helper::cragex;
use crate::utils::sii_parser::{parse_trailers_from_sii, parse_trucks_from_sii};
use std::path::Path;
use tauri::State;
use tauri::command;

#[command]
pub async fn quicksave_game_info(
    profile_state: State<'_, AppProfileState>,
    cache: State<'_, DecryptCache>,
) -> Result<GameDataQuicksave, String> {
    log!("-------------------------------------------");
    log!("Starte quicksave_game_info()");
    log!("-------------------------------------------");

    let profile = require_current_profile(profile_state.clone())?;
    log!("Profil: {}", profile);

    let save = profile_state
        .current_save
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "Kein Save geladen".to_string())?;
    log!("Save: {}", save);

    let path = game_sii_from_save(Path::new(&save));
    log!("Pfad: {:?}", path);

    let content = decrypt_if_needed(&path).map_err(|e| {
        log!("Decrypt Fehler: {}", e);
        e
    })?;

    log!("Datei geladen. Parsing...");

    // Player block
    let re_player = cragex(r"player\s*:\s*([A-Za-z0-9._]+)\s*\{([\s\S]*?)\}")
        .map_err(|e| format!("Regex Fehler PlayerBlock: {}", e))?;

    let caps = re_player
        .captures(&content)
        .ok_or("Player Block nicht gefunden")?;

    let player_id = caps.get(1).map(|m| m.as_str().to_string());
    let player_block = caps.get(2).map(|m| m.as_str()).unwrap_or("");

    log!("Player ID = {:?}", player_id);

    // my_truck
    let player_my_truck = cragex(r"my_truck\s*:\s*([A-Za-z0-9._]+|null)")?
        .captures(player_block)
        .and_then(|c| {
            let v = c.get(1).unwrap().as_str().trim().to_string();
            if v == "null" { None } else { Some(v) }
        });
    log!("my_truck = {:?}", player_my_truck);

    // my_trailer
    let player_my_trailer = cragex(r"my_trailer\s*:\s*([A-Za-z0-9._]+|null)")?
        .captures(player_block)
        .and_then(|c| {
            let v = c.get(1).unwrap().as_str().trim().to_string();
            if v == "null" { None } else { Some(v) }
        });
    log!("my_trailer = {:?}", player_my_trailer);

    // XP
    let player_xp = cragex(r"experience_points:\s*(\d+)")?
        .captures(&content)
        .and_then(|c| c.get(1).unwrap().as_str().parse::<i64>().ok());
    log!("XP = {:?}", player_xp);

    // Bank block
    let re_bank = cragex(r"bank\s*:\s*([A-Za-z0-9._]+)\s*\{([\s\S]*?)\}")
        .map_err(|e| format!("Regex Fehler BankBlock: {}", e))?;
    let caps_bank = re_bank
        .captures(&content)
        .ok_or("Bank Block nicht gefunden")?;
    let bank_id = caps_bank.get(1).map(|m| m.as_str().to_string());
    let bank_block = caps_bank.get(2).map(|m| m.as_str()).unwrap_or("");
    log!("Bank ID = {:?}", bank_id);

    // Player money (_player_money bleibt unverändert)
    let _player_money = cragex(r"money_account:\s*(\d+)")?
        .captures(bank_block)
        .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok());
    log!("Money geladen: {:?}", _player_money);

    // Skills helper
    let skill = |name: &str| -> Option<i64> {
        cragex(&format!(r"{}:\s*(\d+)", name))
            .ok()?
            .captures(&content)
            .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok())
    };

    let adr = skill("adr");
    let long_dist = skill("long_dist");
    let heavy = skill("heavy");
    let fragile = skill("fragile");
    let urgent = skill("urgent");
    let mechanical = skill("mechanical");
    log!("Skills geladen");

    // All trucks
    let trucks = parse_trucks_from_sii(&content);
    log!("{} Trucks gefunden", trucks.len());

    // Player truck data
    let mut truck_brand = None;
    let mut truck_model = None;
    let mut license_plate = None;
    let mut odometer = None;
    let mut trip_fuel_l = None;

    if let Some(ref id) = player_my_truck {
        let id_clean = id.trim().to_lowercase();
        if let Some(t) = trucks
            .iter()
            .find(|t| t.truck_id.to_lowercase() == id_clean)
        {
            truck_brand = Some(t.brand.clone());
            truck_model = Some(t.model.clone());

            // Vehicle block
            let vehicle_regex = format!(
                r"vehicle\s*:\s*{}\s*\{{([\s\S]*?)\}}",
                regex::escape(&t.truck_id)
            );
            let re_vehicle =
                cragex(&vehicle_regex).map_err(|e| format!("Regex Fehler Vehicle Block: {}", e))?;
            if let Some(caps) = re_vehicle.captures(&content) {
                let block = caps.get(1).map(|m| m.as_str()).unwrap_or("");

                odometer = cragex(r"odometer:\s*(\d+)")?
                    .captures(block)
                    .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok());

                trip_fuel_l = cragex(r"trip_fuel_l:\s*(\d+)")?
                    .captures(block)
                    .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok());

                license_plate = cragex(r#"license_plate:\s*"(.+?)""#)?
                    .captures(block)
                    .map(|c| c.get(1).map(|m| m.as_str().to_string()))
                    .flatten();
            }
        }
    }

    // TRAILER PARSING
    let trailers = parse_trailers_from_sii(&content);
    log!("{} Trailer gefunden", trailers.len());

    let mut trailer_brand = None;
    let mut trailer_model = None;
    let mut trailer_license_plate = None;
    let mut trailer_odometer: Option<Vec<f32>> = None;
    // Initialize these to None as well
    let mut trailer_odometer_float: Option<Vec<f32>> = None;
    let mut trailer_wear_float: Option<Vec<f32>> = None;
    let mut trailer_wheels_float: Option<Vec<String>> = None;

    if let Some(ref my_trailer_id) = player_my_trailer {
        let id_clean = my_trailer_id.trim().to_lowercase();
        // The variable 'tr' is bound here
        if let Some(tr) = trailers
            .iter()
            .find(|t| t.trailer_id.to_lowercase() == id_clean)
        {
            // These assignments are inside the scope of 'tr'
            trailer_brand = tr.brand.clone();
            trailer_model = tr.model.clone();
            trailer_license_plate = tr.license_plate.clone();
            trailer_odometer = Some(vec![tr.odometer]);
            // The problematic lines moved inside here:
            trailer_odometer_float = tr.odometer_float.map(|v| vec![v]);
            trailer_wear_float = tr.wear_float.map(|v| vec![v]);
            trailer_wheels_float = tr
                .wheels_float
                .clone()
                .map(|vec_f32| vec_f32.into_iter().map(|f| f.to_string()).collect());
        }
    }

    Ok(GameDataQuicksave {
        player_id,
        bank_id,
        player_xp,
        player_my_truck: player_my_truck.clone(),
        player_my_trailer,
        adr,
        long_dist,
        heavy,
        fragile,
        urgent,
        mechanical,
        vehicle_id: player_my_truck.clone(),
        brand_path: truck_brand.clone(),
        license_plate,
        odometer,
        trip_fuel_l,
        truck_brand,
        truck_model,
        trailer_brand,
        trailer_model,
        trailer_license_plate,
        trailer_odometer,
        trailer_odometer_float,
        trailer_wear_float,
        trailer_wheels_float,
    })
}
