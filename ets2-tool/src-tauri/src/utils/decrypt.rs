use crate::log;
use std::fs;
use std::path::Path;
use std::process::Command;
use decrypt_truck::decrypt_bin_file;

/// Liest eine .sii-Datei und entschlüsselt sie bei Bedarf
pub fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    log!("decrypt_if_needed (rust): {}", path.display());

    let bytes = fs::read(path)
        .map_err(|e| format!("Datei konnte nicht gelesen werden: {}", e))?;

    // Wenn Datei bereits Klartext ist → direkt zurück
    if bytes.starts_with(b"SiiNunit") {
        log!("Datei ist bereits entschlüsselt");
        return Ok(String::from_utf8_lossy(&bytes).to_string());
    }

    // ETS2 Save → binär verschlüsselt → jetzt entschlüsseln
    let decrypted = decrypt_bin_file(&bytes)
        .map_err(|e| format!("Decrypt fehlgeschlagen: {:?}", e))?;

    Ok(String::from_utf8_lossy(&decrypted).to_string())
}

/// Erstellt ein Backup der Originaldatei als .bak
pub fn backup_file(path: &Path) -> Result<(), String> {
    let backup_path = path.with_extension("bak");
    fs::copy(path, &backup_path).map_err(|e| e.to_string())?;
    log!("Backup erstellt: {}", backup_path.display());
    Ok(())
}

/// Modifiziert einen Block in der Originaldatei atomar
pub fn modify_block(
    path: &Path,
    block_name: &str,
    updater: impl Fn(&str) -> String,
) -> Result<(), String> {
    backup_file(path)?;

    // Originaldatei lesen
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;

    // Regex für Block
    let re = regex::Regex::new(&format!(
        r"{}s*:\s*[A-Za-z0-9._]+\s*\{{([\s\S]*?)\}}",
        block_name
    ))
    .map_err(|e| e.to_string())?;

    // Block modifizieren
    let new_content = re
        .replace(&content, |caps: &regex::Captures| updater(&caps[1]))
        .to_string();

    // Atomar schreiben über Temp-Datei
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, &new_content).map_err(|e| e.to_string())?;
    fs::rename(tmp_path, path).map_err(|e| e.to_string())?;

    log!(
        "Block '{}' erfolgreich modifiziert: {}",
        block_name,
        path.display()
    );
    Ok(())
}
