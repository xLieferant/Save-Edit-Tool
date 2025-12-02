use crate::log; // This import is now used
use crate::models::quicksave_game_info::GameDataQuicksave;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::paths::quicksave_game_path;
use regex::Regex;
use std::env;
use tauri::command;

#[command]
pub fn read_all_save_data() -> Result<GameDataQuicksave, String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| {
        log!("Error: Kein Profil geladen."); // Used here
        "Kein Profil geladen.".to_string()
    })?;

    log!("Lese alle Speicherdaten aus Profil: {}", profile); // Used here
    let path = quicksave_game_path(&profile);
    let content = decrypt_if_needed(&path)?;

    let re = |pat: &str| Regex::new(pat).unwrap();
    let data = GameDataQuicksave {
        money: re(r"info_money_account:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        xp: re(r"info_players_experience:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        recruitments: re(r"info_unlocked_recruitments:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        dealers: re(r"info_unlocked_dealers:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        visited_cities: re(r"info_visited_cities:\s*(\d+)")
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
    };
    log!(
        "Gefundene Daten: Geld: {:?}, XP: {:?}, Recruitments: {:?}, dealers: {:?}, visited_cities: {:?}",
        data.money,
        data.xp,
        data.recruitments,
        data.dealers,
        data.visited_cities,
    );
    Ok(data)
}