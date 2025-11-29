use tauri::command;
use crate::models::save_game_data::SaveGameData;
use crate::utils::paths::autosave_path;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::extract::extract_value;

/// Lies den money_account-Wert aus dem Autosave des angegebenen Profiles.
#[command]
pub fn read_money(profile_path: String) -> Result<i64, String> {
    let autosave = autosave_path(&profile_path);

    if !autosave.exists() {
        return Err(format!("Autosave nicht gefunden: {}", autosave.display()));
    }

    let txt = decrypt_if_needed(&autosave)?;
    extract_value(&txt, "money_account").ok_or_else(|| "money_account nicht gefunden".to_string())
}

/// Lies den experience_points-Wert aus dem Autosave des angegebenen Profiles.
#[command]
pub fn read_xp(profile_path: String) -> Result<i64, String> {
    let autosave = autosave_path(&profile_path);

    if !autosave.exists() {
        return Err(format!("Autosave nicht gefunden: {}", autosave.display()));
    }

    let txt = decrypt_if_needed(&autosave)?;
    extract_value(&txt, "experience_points").ok_or_else(|| "experience_points nicht gefunden".to_string())
}

/// Liest einige zentrale Werte und gibt ein SaveGameData zurück.
/// Fallback: setzt 0 für nicht gefundene Werte.
#[command]
pub fn read_all_save_data(profile_path: String) -> Result<SaveGameData, String> {
    let autosave = autosave_path(&profile_path);

    if !autosave.exists() {
        return Err(format!("Autosave nicht gefunden: {}", autosave.display()));
    }

    let txt = decrypt_if_needed(&autosave)?;

    let money = extract_value(&txt, "money_account").unwrap_or(0);
    let xp = extract_value(&txt, "experience_points").unwrap_or(0);
    let level = extract_value(&txt, "info_player_level").unwrap_or(0);

    Ok(SaveGameData { money, xp, level })
}
