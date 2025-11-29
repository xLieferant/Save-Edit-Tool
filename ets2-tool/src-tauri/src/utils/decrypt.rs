use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    if let Ok(content) = fs::read_to_string(path) {
        if content.starts_with("SiiNunit") {
            return Ok(content);
        }
    }

    let out = std::env::temp_dir()
        .join("ets2_tool")
        .join(format!(
            "decoded_{}.sii",
            path.file_stem().unwrap_or_default().to_string_lossy()
        ));

    let _ = fs::create_dir_all(out.parent().unwrap());

    let decrypt_local = PathBuf::from("tools/SII_Decrypt.exe");
    let mut success = false;

    if decrypt_local.exists() {
        if Command::new(&decrypt_local).arg(path).arg(&out).output().is_ok() {
            if out.exists() {
                success = true;
            }
        }
    }

    if !success {
        if Command::new("decrypt_truck").arg(path).arg(&out).output().is_ok() {
            if out.exists() {
                success = true;
            }
        }
    }

    if success {
        return fs::read_to_string(&out)
            .map_err(|e| format!("Fehler beim Lesen decrypted: {}", e));
    }

    fs::read_to_string(path)
        .map_err(|e| format!("Fehler beim Lesen: {}", e))
}
