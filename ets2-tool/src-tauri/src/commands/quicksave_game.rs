use crate::log;
use crate::models::quicksave_game_info::GameDataQuicksave;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::quicksave_game_path;
use regex::Regex;
use std::env;
use tauri::command;
use std::collections::HashMap;

#[command]
pub fn quicksave_game_info() -> Result<GameDataQuicksave, String> {
    log!("Lese Quicksave Game.sii");

    let profile = env::var("CURRENT_PROFILE").map_err(|_| {
        log!("Error: Kein Profil geladen.");
        "Kein Profil geladen.".to_string()
    })?;

    let path = quicksave_game_path(&profile);
    let content = decrypt_if_needed(&path)?;

    let re = |pat: &str| Regex::new(pat).unwrap();

    let adr = re(r"adr:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok());
    let long_dist = re(r"long_dist:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok());
    let heavy = re(r"heavy:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok());
    let fragile = re(r"fragile:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok());
    let urgent = re(r"urgent:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok());
    let mechanical = re(r"mechanical:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok());

    let vehicle_id = re(r"vehicle\s*:\s*([a-zA-Z0-9._]+)")
        .captures(&content)
        .map(|c| c[1].to_string());

    // Accessories sammeln
    let mut accessory_map = HashMap::<String, String>::new();
    let re_acc = Regex::new(
        r#"vehicle_accessory\s*:\s*([a-zA-Z0-9._]+)\s*\{[^}]*?data_path:\s*"([^"]+)""#
    ).unwrap();

    for cap in re_acc.captures_iter(&content) {
        accessory_map.insert(cap[1].to_string(), cap[2].to_string());
    }

    // Marke / Model via data_path
    let mut brand_path: Option<String> = None;

    if let Some(ref v_id) = vehicle_id {
        let block_re = Regex::new(&format!(
            r"vehicle\s*:\s*{}\s*\{{([^}}]+)}}",
            regex::escape(v_id)
        )).unwrap();

        if let Some(cap) = block_re.captures(&content) {
            let block = cap[1].to_string();

            let re_block_acc = Regex::new(r"accessories\[\d+\]\s*:\s*([a-zA-Z0-9._]+)").unwrap();
            for ac in re_block_acc.captures_iter(&block) {
                let id = ac[1].to_string();
                if let Some(path) = accessory_map.get(&id) {
                    if path.contains("/def/vehicle/truck/") {
                        brand_path = Some(path.clone());
                    }
                }
            }
        }
    }

    let license_plate = re(r#"license_plate:\s*"([^"]+)""#)
        .captures(&content)
        .map(|c| c[1].to_string());

    let odometer = re(r"odometer:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok());
    let trip_fuel_l =
        re(r"trip_fuel_l:\s*(\d+)").captures(&content).and_then(|c| c[1].parse().ok());

    let data = GameDataQuicksave {
        adr,
        long_dist,
        heavy,
        fragile,
        urgent,
        mechanical,

        vehicle_id,
        brand_path,
        license_plate,
        odometer,
        trip_fuel_l,
    };

    log!("Quicksave Daten extrahiert.");

    Ok(data)
}
