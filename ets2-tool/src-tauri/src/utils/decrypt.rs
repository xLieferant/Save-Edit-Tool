use std::fs;
use std::path::Path;
use std::process::Command;
use crate::log;

/// Entschlüsselt nur .sii-Dateien, z.B. info.sii oder profile.sii
pub fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    log!("decrypt_if_needed: {}", path.display());

    // 1. Nur .sii-Dateien entschlüsseln
    if path.extension().map(|ext| ext != "sii").unwrap_or(true) {
        log!("Datei ist keine .sii, kein Entschlüsseln nötig.");
        return fs::read_to_string(path)
            .map_err(|e| format!("Fehler beim Lesen der Datei: {}", e));
    }

    // 2. Prüfen, ob schon entschlüsselt
    if let Ok(orig) = fs::read_to_string(path) {
        if orig.starts_with("SiiNunit") {
            log!("Datei ist bereits entschlüsselt.");
            return Ok(orig);
        }
    }

    // 3. Temp-Zielpfad für entschlüsselte Dateien
    let temp_out = std::env::temp_dir()
        .join("ets2_tool")
        .join(format!("decoded_{}.sii", path.file_stem().unwrap().to_string_lossy()));

    let _ = fs::create_dir_all(temp_out.parent().unwrap());

    // 4. Falls Temp-Datei existiert: löschen, damit wir immer frisch entschlüsseln
    if temp_out.exists() {
        log!("Lösche alte Temp-Datei: {}", temp_out.display());
        let _ = fs::remove_file(&temp_out);
    }

    // 5. Entschlüsseln mit Tools
    let local_tool = Path::new("tools/SII_Decrypt.exe");
    let mut decrypted = false;

    // 6. Versuche lokales Tool
    if local_tool.exists() {
        log!("Versuche tools/SII_Decrypt.exe für {}", path.display());
        if let Ok(output) = Command::new(&local_tool).arg(path).arg(&temp_out).output() {
            if output.status.success() && temp_out.exists() {
                decrypted = true;
                log!("Datei erfolgreich entschlüsselt (tools/SII_Decrypt.exe).");
            } else {
                log!(
                    "tools/SII_Decrypt.exe fehlgeschlagen: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }

    // 7. Fallback decrypt_truck
    if !decrypted {
        log!("Fallback: decrypt_truck für {}", path.display());
        if let Ok(output) = Command::new("decrypt_truck").arg(path).arg(&temp_out).output() {
            if output.status.success() && temp_out.exists() {
                decrypted = true;
                log!("Datei erfolgreich entschlüsselt (decrypt_truck).");
            } else {
                log!(
                    "decrypt_truck fehlgeschlagen: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }

    // 8. Ergebnis
    if decrypted {
        fs::read_to_string(&temp_out)
            .map_err(|e| format!("Fehler beim Lesen der entschlüsselten Datei: {}", e))
    } else {
        fs::read_to_string(path)
            .map_err(|e| format!("Fehler beim Lesen der Originaldatei: {}", e))
    }
}
