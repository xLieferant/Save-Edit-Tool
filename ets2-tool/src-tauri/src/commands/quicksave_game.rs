use crate::log; // This import is now used
use crate::models::quicksave_game_info::GameDataQuicksave;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::quicksave_game_path;
use regex::Regex;
use std::env;
use tauri::command;
use std::fs;

#[command]
pub fn quicksave_game_info() -> Result<GameDataQuicksave, String> {

    log!("Lese Quicksave Game.sii");
    
    let profile = env::var("CURRENT_PROFILE").map_err(|_| {
        log!("Error: Kein Profil geladen."); // Used here
        "Kein Profil geladen.".to_string()
    })?;

    log!("Lese alle Speicherdaten aus Profil: {}", profile); // Used here
    let path = quicksave_game_path(&profile);
    let content = decrypt_if_needed(&path)?;

    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = GameDataQuicksave {
        adr: re(r"adr:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        long_dist: re(r"long_dist:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        heavy: re(r"heavy:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        fragile: re(r"fragile:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        urgent: re(r"urgent:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        mechanical: re(r"mechanical:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        vehicle: re(r"vehicle\s*:\s*([a-zA-Z0-9._]+)") // <-- Hinzugefügte Klammer hier
            .captures(&content)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string())),
    };
    log!(
        "Gefundene Daten: ADR: {:?}, Long Distance: {:?}, Heavy load: {:?}, Fragile: {:?}, urgend: {:?}, mechanical: {:?}, vehicle: {:?}",
        data.adr,
        data.long_dist,
        data.heavy,
        data.fragile,
        data.urgent,
        data.mechanical,
        data.vehicle,
    );
    Ok(data)
}


/* Ich will noch diesen Command hier auslesen  
license_plate: "B<offset hshift=4 vshift=-5><img src=/material/ui/lp/germany/b.mat color=FFFFFFFF><offset hshift=4 vshift=5>RG 4117|germany"

Bestimmt sowas wie - finde vehicle : _nameless.1df.04bc.8810 
.... 
odometer: 7256
trip_fuel_l: 1050

 */