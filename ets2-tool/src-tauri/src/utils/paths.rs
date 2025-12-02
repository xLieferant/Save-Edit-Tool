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
      Path::new(profile_dir)
          .join("config.cfg")
  }

/// Pfad zur globalen config.cfg (Basis-Verzeichnis)
pub fn ets2_base_config_path() -> Option<PathBuf> {
    ets2_base_path().map(|base_path| base_path.join("config.cfg"))
}