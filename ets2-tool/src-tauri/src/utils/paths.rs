use std::env;
use std::path::{Path, PathBuf};

pub fn ets2_base_path() -> Option<PathBuf> {
    dirs::document_dir().map(|d| d.join("Euro Truck Simulator 2"))
}

pub fn autosave_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path)
        .join("save")
        .join("quicksave")
        .join("info.sii")
}

pub fn quicksave_game_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path)
        .join("save")
        .join("quicksave")
        .join("game.sii")
}

pub fn quicksave_config_path(profile_dir: &str) -> PathBuf {
    Path::new(profile_dir).join("config.cfg")
}

/// Pfad zur globalen config.cfg (Basis-Verzeichnis)
pub fn ets2_base_config_path() -> Option<PathBuf> {
    ets2_base_path().map(|base_path| base_path.join("config.cfg"))
}

/* --------------------------------------------------
   WRAPPER FÜR APPLY_SETTING
-------------------------------------------------- */

pub fn autosave_path_current() -> Result<PathBuf, String> {
    let profile = env::var("CURRENT_PROFILE").map_err(|_| "CURRENT_PROFILE not set".to_string())?;

    Ok(PathBuf::from(profile).join("save").join("autosave.sii"))
}

pub fn base_config_path() -> Result<PathBuf, String> {
    ets2_base_config_path().ok_or_else(|| "Could not resolve ETS2 base config path".to_string())
}
