use std::path::{Path, PathBuf};

pub fn autosave_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path)
        .join("save")
        .join("autosave")
        .join("info.sii")
}

pub fn ets2_base_path() -> Option<PathBuf> {
    dirs::document_dir().map(|d| d.join("Euro Truck Simulator 2"))
}
