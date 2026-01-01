use crate::log;
use crate::state::DecryptCache;
use decrypt_truck::decrypt_bin_file;
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::State;

/// Liest eine .sii-Datei und entschlüsselt sie bei Bedarf

pub fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    log!("decrypt_if_needed: {}", path.display());

    let bytes = fs::read(path).map_err(|e| format!("Datei konnte nicht gelesen werden: {}", e))?;

    // 1️⃣ Klartext?
    if bytes.starts_with(b"SiiNunit") {
        return Ok(String::from_utf8_lossy(&bytes).to_string());
    }

    // 2️⃣ Nicht-binäre Datei? → einfach als Text lesen
    if !bytes.starts_with(b"\x00") && !bytes.iter().any(|b| *b == 0) {
        return Ok(String::from_utf8_lossy(&bytes).to_string());
    }

    // 3️⃣ ETS2 verschlüsselt → decrypt_truck
    match decrypt_bin_file(&bytes) {
        Ok(decrypted) => Ok(String::from_utf8_lossy(&decrypted).to_string()),
        Err(e) => {
            log!("Decrypt übersprungen ({}): {:?}", path.display(), e);
            // Fallback: als Text zurückgeben
            Ok(String::from_utf8_lossy(&bytes).to_string())
        }
    }
}

pub fn decrypt_cached(path: &Path, cache: &State<DecryptCache>) -> Result<String, String> {
    // Cache hit?
    if let Some(v) = cache.files.lock().unwrap().get(path).cloned() {
        return Ok(v);
    }

    // Decrypt einmal
    let content = decrypt_if_needed(path)?;

    // Cache speichern
    cache
        .files
        .lock()
        .unwrap()
        .insert(path.to_path_buf(), content.clone());

    Ok(content)
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
