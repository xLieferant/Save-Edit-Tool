use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::models::save_game_data::SaveGameData;
use crate::utils::{decrypt::decrypt_if_needed, paths::autosave_path};

use tauri::command;

#[command]
pub fn read_money(profile_path: String) -> Result<i64, String> {
    let autosave = autosave_path(&profile_path);

    let txt = decrypt_if_needed(&autosave)?;
    let money_line = txt
        .lines()
        .find(|l| l.contains("money_account"))
        .ok_or("money_account nicht gefunden")?;

    let value = money_line
        .split(':')
        .nth(1)
        .ok_or("Ungültiges money_account Format")?
        .trim()
        .parse::<i64>()
        .map_err(|_| "Fehler beim Parsen von money_account")?;

    Ok(value)
}

#[command]
pub fn read_xp(profile_path: String) -> Result<i64, String> {
    let autosave = autosave_path(&profile_path);

    let txt = decrypt_if_needed(&autosave)?;
    let xp_line = txt
        .lines()
        .find(|l| l.contains("experience_points"))
        .ok_or("experience_points nicht gefunden")?;

    let value = xp_line
        .split(':')
        .nth(1)
        .ok_or("Ungültiges XP Format")?
        .trim()
        .parse::<i64>()
        .map_err(|_| "Fehler beim Parsen von XP")?;

    Ok(value)
}

#[command]
pub fn read_all_save_data(profile_path: String) -> Result<SaveGameData, String> {
    let autosave = autosave_path(&profile_path);
    let txt = decrypt_if_needed(&autosave)?;

    let money = txt
        .lines()
        .find(|l| l.contains("money_account"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(0);

    let xp = txt
        .lines()
        .find(|l| l.contains("experience_points"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(0);

    let level = txt
        .lines()
        .find(|l| l.contains("user_xp"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(0);

    Ok(SaveGameData { money, xp, level })
}
