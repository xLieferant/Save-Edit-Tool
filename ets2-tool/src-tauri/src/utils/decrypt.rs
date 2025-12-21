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

/// Sichert die Originaldatei als .bak
pub fn backup_file(path: &Path) -> Result<(), String> {
    let backup_path = path.with_extension("bak");
    fs::copy(path, &backup_path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Modifiziert einen Block in der Datei und ersetzt die Originaldatei atomar
pub fn modify_block(path: &Path, block_name: &str, updater: impl Fn(&str) -> String) -> Result<(), String> {
    // Backup erstellen
    backup_file(path)?;

    // Datei lesen
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;

    // Regex zum Finden des Blocks
    let re = regex::Regex::new(&format!(r"{}s*:\s*[A-Za-z0-9._]+\s*\{{([\s\S]*?)\}}", block_name))
        .map_err(|e| e.to_string())?;

    // Block modifizieren
    let new_content = re.replace(&content, |caps: &regex::Captures| {
        let block = &caps[1];
        updater(block)
    }).to_string();

    // Temp-Datei schreiben
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, &new_content).map_err(|e| e.to_string())?;

    // Atomar ersetzen
    fs::rename(tmp_path, path).map_err(|e| e.to_string())?;

    Ok(())
}