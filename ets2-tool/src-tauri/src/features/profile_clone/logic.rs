use crate::models::clone_profiles_info::CloneOptions;
use crate::utils::{decrypt::decrypt_if_needed, hex};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::{ZipWriter, write::FileOptions};
use zip::CompressionMethod;
use std::io::Write;

/// Hauptlogik
pub fn clone_profile(
    source: &Path,
    new_name: &str,
    options: CloneOptions,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !source.exists() {
        return Err("Quellprofil existiert nicht".into());
    }

    let parent = source.parent().ok_or("Kein Parent-Verzeichnis")?;
    let new_hex = hex::text_to_hex(new_name);
    let target_dir = parent.join(&new_hex);

    if target_dir.exists() {
        return Err("Zielprofil existiert bereits".into());
    }

    // 1️⃣ ZIP-Backup
    if options.backup {
        create_zip_backup(source, parent)?;
    }

    // 2️⃣ Temp kopieren
    let temp_dir = parent.join(format!("{}_tmp", new_hex));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    copy_dir_recursive(source, &temp_dir)?;

    // 3️⃣ Alten Namen aus profile.sii lesen
    let profile_sii = temp_dir.join("profile.sii");
    let decrypted = decrypt_if_needed(&profile_sii)?;
    let old_name =
        extract_profile_name(&decrypted).ok_or("Profilname konnte nicht gelesen werden")?;

    // 4️⃣ Ersetzen
    replace_identifiers(&temp_dir, &old_name, new_name, options)?;

    // 5️⃣ Final umbenennen
    fs::rename(&temp_dir, &target_dir)?;

    Ok(target_dir)
}

fn create_zip_backup(source: &Path, parent: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let backup_root = parent.join("Save Edit Tool Profile Backups");
    fs::create_dir_all(&backup_root)?;

    let profile_name = source.file_name().unwrap().to_string_lossy();
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let zip_path = backup_root.join(format!("{}_{}.zip", profile_name, timestamp));

    let file = File::create(zip_path)?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<()> = 
        FileOptions::default().compression_method(CompressionMethod::Deflated);

    for entry in WalkDir::new(source) {
        let entry = entry?;
        let path = entry.path();
        let name = path.strip_prefix(source)?.to_string_lossy();

        if path.is_file() {
            zip.start_file(name, options)?;
            let data = fs::read(path)?;
            zip.write_all(&data)?;
        }
    }

    zip.finish()?;
    Ok(())
}

/// Rekursives Kopieren
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let dest = dst.join(rel);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest)?;
        } else {
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// Profilname aus profile.sii extrahieren
fn extract_profile_name(content: &str) -> Option<String> {
    for line in content.lines() {
        if line.trim_start().starts_with("profile_name:") {
            return line.split('"').nth(1).map(|s| s.to_string());
        }
    }
    None
}

/// Ersetzungen durchführen
fn replace_identifiers(
    dir: &Path,
    old: &str,
    new: &str,
    options: CloneOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let old_hex = hex::text_to_hex(old);
    let new_hex = hex::text_to_hex(new);

    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let mut content = decrypt_if_needed(path)?;

        if options.replace_text {
            content = content.replace(old, new);
        }

        if options.replace_hex {
            content = content.replace(&old_hex, &new_hex);
        }

        fs::write(path, content)?;
    }

    Ok(())
}
