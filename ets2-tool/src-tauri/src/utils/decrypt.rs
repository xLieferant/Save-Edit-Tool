use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Entschlüsselt eine SII-Datei falls nötig.
/// Rückgabe: Klartext oder Fehler-String.
pub fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    // Erst kurz probelesen — wenn schon Klartext (SiiNunit...), return
    if let Ok(content) = fs::read_to_string(path) {
        if content.starts_with("SiiNunit") {
            return Ok(content);
        }
    }

    // Temp-Ausgabedatei
    let out = std::env::temp_dir()
        .join("ets2_tool")
        .join(format!(
            "decoded_{}.sii",
            path.file_stem().unwrap_or_default().to_string_lossy()
        ));

    let _ = fs::create_dir_all(out.parent().unwrap());

    // Versuche lokale tools/SII_Decrypt.exe
    let try_local = PathBuf::from("tools/SII_Decrypt.exe");
    let mut did = false;

    if try_local.exists() {
        if let Ok(output) = Command::new(&try_local).arg(path).arg(&out).output() {
            if output.status.success() && out.exists() {
                did = true;
            }
        }
    }

    // Fallback: global decrypt_truck
    if !did {
        if let Ok(output) = Command::new("decrypt_truck").arg(path).arg(&out).output() {
            if output.status.success() && out.exists() {
                did = true;
            }
        }
    }

    if did && out.exists() {
        return fs::read_to_string(&out)
            .map_err(|e| format!("Fehler beim Lesen der entschlüsselten Datei: {}", e));
    }

    // Letzter Fallback: Original lesen (kann verschlüsselt sein)
    fs::read_to_string(path).map_err(|e| format!("Fehler beim Lesen der Datei: {}", e))
}
