use std::path::{Path, PathBuf};
use std::fs;
use crate::models::clone_profiles_info::CloneOptions;
use crate::utils::hex; // dein vorhandenes hex_to_text / text_to_hex
use crate::utils::decrypt::decrypt_if_needed;
use uuid::Uuid;
use walkdir::WalkDir;
use std::env::temp_dir;

/// Hauptfunktion, die alles orchestriert
pub fn clone_profile(
    source: &Path,
    new_name: &str,
    options: CloneOptions
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !source.exists() {
        return Err("Quellprofil existiert nicht".into());
    }

    // 1️⃣ Backup
    if options.backup {
        let backup_path = source.with_extension("backup.zip");
        backup_profile(source, &backup_path)?;
        println!("Backup erstellt: {:?}", backup_path);
    }

    // // 2️⃣ Neues Profilverzeichnis 
    // let new_id = hex::encode({Uuid::new_v4().to_string()});  // #FIXME <- Hex to text ! 
    // let parent_dir = source.parent().ok_or("Kein übergeordnetes Verzeichnis")?;
    // let temp_dir = parent_dir.join(format!("{}_tmp", new_id));
    // fs::create_dir_all(&temp_dir)?;
    // copy_dir_recursive(source, &temp_dir)?;

    // 3️⃣ Inhalte anpassen
    replace_identifiers(&temp_dir, source.file_name().unwrap().to_str().unwrap(), new_name, options)?;

    // 4️⃣ Final umbenennen
    let final_path = parent_dir.join(new_name);
    fs::rename(&temp_dir, &final_path)?;

    Ok(final_path)
}

/// Backup als ZIP erstellen
fn backup_profile(source: &Path, zip_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Platzhalter, du kannst dein bestehendes ZIP-Backup nutzen
    // fs::copy einfach als Platzhalter:
    fs::copy(source, zip_path)?;
    Ok(())
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
    options: CloneOptions
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

                        let old_escaped: String = old_name.as_bytes().iter().map(|b| format!("\\x{:02x}", b)).collect();
                        let new_escaped: String = new_name.as_bytes().iter().map(|b| format!("\\x{:02x}", b)).collect();
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
