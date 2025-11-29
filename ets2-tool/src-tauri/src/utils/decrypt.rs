use std::fs;
use std::path::Path;
use std::process::Command;
use crate::log;

pub fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    log!("decrypt_if_needed: {}", path.display());

    if let Ok(orig) = fs::read_to_string(path) {
        if orig.starts_with("SiiNunit") {
            return Ok(orig);
        }
    }

    let out = std::env::temp_dir()
        .join("ets2_tool")
        .join(format!("decoded_{}.sii", path.file_stem().unwrap().to_string_lossy()));

    let _ = fs::create_dir_all(out.parent().unwrap());

    let try_local = Path::new("tools/SII_Decrypt.exe");
    let mut did_decrypt = false;

    if try_local.exists() {
        log!("Versuche tools/SII_Decrypt.exe");
        if let Ok(output) = Command::new(&try_local).arg(path).arg(&out).output() {
            if output.status.success() {
                did_decrypt = true;
                log!("Decrypt +1");
            } else {
                log!("tools/SII_Decrypt.exe fehlgeschlagen: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
    }

    if !did_decrypt {
        if let Ok(output) = Command::new("decrypt_truck").arg(path).arg(&out).output() {
            if output.status.success() {
                did_decrypt = true;
            } else {
                log!("decrypt_truck fehlgeschlagen: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
    }

    if did_decrypt && out.exists() {
        return fs::read_to_string(&out).map_err(|e| format!("Fehler beim Lesen der entschlüsselten Datei: {}", e));
    }

    fs::read_to_string(path).map_err(|e| format!("Fehler beim Lesen der Originaldatei: {}", e))
}
