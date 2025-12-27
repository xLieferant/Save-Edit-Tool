use crate::log;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Entschlüsselt eine .sii-Datei bei Bedarf und gibt den Inhalt zurück.
/// Temp-Dateien werden in %TEMP%/ets2_tool erstellt, Originaldateien bleiben unberührt.
pub fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    log!("decrypt_if_needed: {}", path.display());

    if path.extension().and_then(|ext| ext.to_str()) != Some("sii") {
        log!("Datei ist keine .sii, kein Entschlüsseln nötig.");
        let bytes = fs::read(path).map_err(|e| format!("Fehler beim Lesen der Datei: {}", e))?;
        return Ok(String::from_utf8_lossy(&bytes).to_string());
    }

    let orig_bytes = fs::read(path).map_err(|e| format!("Fehler beim Lesen der Datei: {}", e))?;
    let orig = String::from_utf8_lossy(&orig_bytes);
    if orig.starts_with("SiiNunit") {
        log!("Datei ist bereits entschlüsselt.");
        return Ok(orig.to_string());
    }

    let temp_out = std::env::temp_dir().join("ets2_tool")
        .join(format!("decoded_{}.sii", path.file_stem().unwrap().to_string_lossy()));
    let _ = fs::create_dir_all(temp_out.parent().unwrap());
    let _ = fs::remove_file(&temp_out);

    let decrypted = if Path::new("tools/SII_Decrypt.exe").exists() {
        log!("Versuche tools/SII_Decrypt.exe für {}", path.display());
        let output = Command::new("tools/SII_Decrypt.exe").arg(path).arg(&temp_out).output();
        output.map_or(false, |o| o.status.success() && temp_out.exists())
    } else {
        false
    } || {
        log!("Fallback: decrypt_truck für {}", path.display());
        let output = Command::new("decrypt_truck").arg(path).arg(&temp_out).output();
        output.map_or(false, |o| o.status.success() && temp_out.exists())
    };

    if decrypted {
        let decrypted_bytes = fs::read(&temp_out)
            .map_err(|e| format!("Fehler beim Lesen der entschlüsselten Datei: {}", e))?;
        Ok(String::from_utf8_lossy(&decrypted_bytes).to_string())
    } else {
        Ok(orig.to_string())
    }
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
