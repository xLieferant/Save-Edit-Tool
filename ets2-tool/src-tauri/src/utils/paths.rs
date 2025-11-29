use std::path::{PathBuf, Path};

/// Liefert mögliche ETS2-Suchpfade (Documents/Euro Truck Simulator 2/...).
pub fn resolve_ets2_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(doc) = dirs::document_dir() {
        let base = doc.join("Euro Truck Simulator 2");
        out.push(base.join("profiles"));
        out.push(base.join("profiles.backup"));
        out.push(base);
    }
    out
}

/// Pfad zum Autosave/info.sii ausgehend von profile dir (profile folder path)
pub fn autosave_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path).join("save").join("autosave").join("info.sii")
}
