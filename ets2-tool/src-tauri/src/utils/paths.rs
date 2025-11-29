use std::path::{Path, PathBuf};

pub fn resolve_ets2_paths() -> Vec<PathBuf> {
    let mut result = Vec::new();

    if let Some(doc) = dirs::document_dir() {
        let base = doc.join("Euro Truck Simulator 2");

        result.push(base.join("profiles"));
        result.push(base.join("profiles.backup"));
        result.push(base);
    }

    result
}

pub fn autosave_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path)
        .join("save")
        .join("autosave")
        .join("info.sii")
}
