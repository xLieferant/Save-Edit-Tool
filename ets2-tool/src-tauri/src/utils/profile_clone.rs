use crate::models::clone_profiles_info::CloneOptions;
use crate::utils::decrypt::decrypt_if_needed;
use crate::utils::hex; // dein vorhandenes hex_to_text / text_to_hex
use tauri::command;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::ZipWriter;
use zip::ZipArchive;


/// Hauptfunktion, die alles orchestriert
pub fn clone_profile(
    source: &Path,
    new_name: &str,
    options: CloneOptions,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !source.exists() {
        return Err("Quellprofil existiert nicht".into());
    }

    // 1️⃣ Backup
    if options.backup {
        // Backup als Ordner kopieren (ZIP bräuchte externe Crate)
        let backup_path = source.with_extension("bak");
        if backup_path.exists() {
            fs::remove_dir_all(&backup_path)?;
        }
        fs::create_dir_all(&backup_path)?;
        copy_dir_recursive(source, &backup_path)?;
        println!("Backup erstellt: {:?}", backup_path);
    }

    // 2️⃣ Neues Profilverzeichnis vorbereiten
    // Der Ordnername muss der HEX-Wert des neuen Namens sein
    let new_folder_name = hex::text_to_hex(new_name);
    let parent_dir = source.parent().ok_or("Kein übergeordnetes Verzeichnis")?;

    let final_path = parent_dir.join(&new_folder_name);
    if final_path.exists() {
        return Err(format!("Profilordner existiert bereits: {}", new_folder_name).into());
    }

    // Temp-Verzeichnis erstellen
    let temp_dir = parent_dir.join(format!("{}_tmp", new_folder_name));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    // Inhalt kopieren
    copy_dir_recursive(source, &temp_dir)?;

    // 3️⃣ Inhalte anpassen
    // Wir müssen den alten Profilnamen wissen (Text), um ihn zu ersetzen.
    // Wenn der Quellordner Hex ist, decodieren wir ihn.
    let dir_name = source.file_name().unwrap().to_str().unwrap();
    let old_name = hex::decode_hex_folder_name(dir_name).unwrap_or(dir_name.to_string());

    replace_identifiers(&temp_dir, &old_name, new_name, options)?;

    // 4️⃣ Final umbenennen
    fs::rename(&temp_dir, &final_path)?;

    Ok(final_path)
}

/// Ordner rekursiv kopieren
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(src)?;
        let dest_path = dest.join(relative);

        if path.is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else {
            fs::copy(path, &dest_path)?;
        }
    }
    Ok(())
}

/// Inhalte anpassen (Text + Hex)
fn replace_identifiers(
    dir: &Path,
    old_name: &str,
    new_name: &str,
    options: CloneOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let extensions = vec!["sii", "profile", "cfg", "txt", "save"];

    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    // 🔥 WICHTIG: decrypt_if_needed nutzen, sonst crasht es bei binären profile.sii
                    let content = decrypt_if_needed(path)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

                    let mut new_content = content.clone();

                    if options.replace_text {
                        new_content = new_content.replace(old_name, new_name);
                    }

                    if options.replace_hex {
                        let old_hex = hex::text_to_hex(old_name);
                        let new_hex = hex::text_to_hex(new_name);
                        new_content = new_content.replace(&old_hex, &new_hex);

                        let old_escaped: String = old_name
                            .as_bytes()
                            .iter()
                            .map(|b| format!("\\x{:02x}", b))
                            .collect();
                        let new_escaped: String = new_name
                            .as_bytes()
                            .iter()
                            .map(|b| format!("\\x{:02x}", b))
                            .collect();
                        new_content = new_content.replace(&old_escaped, &new_escaped);
                    }

                    if new_content != content {
                        fs::write(path, new_content)?;
                    }
                }
            }
        }
    }

    Ok(())
}

#[command]
pub fn validate_clone_target_cmd(source: String, new_name: String) -> Result<(), String> {
    let source_path = std::path::PathBuf::from(source);

    if !source_path.exists() {
        return Err("Source profile does not exist".into());
    }

    let parent = source_path.parent().ok_or("Invalid source profile path")?;

    let new_folder = crate::utils::hex::text_to_hex(&new_name);
    let target_path = parent.join(new_folder);

    if target_path.exists() {
        return Err("A profile with this name already exists".into());
    }

    Ok(())
}

/// Wrapper-Command für Tauri, da clone_profile selbst nicht direkt aufrufbar ist
#[command]
pub fn clone_profile_cmd(
    source: String,
    new_name: String,
    options: CloneOptions,
) -> Result<String, String> {
    let source_path = PathBuf::from(source);
    match clone_profile(&source_path, &new_name, options) {
        Ok(path) => Ok(path.to_string_lossy().to_string()),
        Err(e) => Err(e.to_string()),
    }
}
