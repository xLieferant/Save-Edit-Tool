use tauri::command;
use std::path::Path;
use std::fs;
use fs_extra::dir::copy;
use crate::dev_log;


#[command]
pub fn copy_profile_controls(
    source_profile_path: String,
    target_profile_path: String,
) -> Result<String, String> {
    let source_path = Path::new(&source_profile_path);
    let target_path = Path::new(&target_profile_path);

    if !source_path.exists() {
        return Err("Quell-Profilpfad existiert nicht".into());
    }

    if !target_path.exists() {
        return Err("Ziel-Profilpfad existiert nicht".into());
    }

    let source_controls = source_path.join("controls.sii");
    if !source_controls.exists() {
        return Err("controls.sii im Quellprofil nicht gefunden".into());
    }

    let target_controls = target_path.join("controls.sii");
    let target_backup = target_path.join("controls_backup.sii");

    dev_log!(
        "Kopiere controls.sii von {} nach {}",
        source_controls.display(),
        target_controls.display()
    );

    // Backup im Ziel anlegen (falls vorhanden)
    if target_controls.exists() {
        fs::copy(&target_controls, &target_backup)
            .map_err(|e| format!("Backup fehlgeschlagen: {}", e))?;
    }

    // Controls kopieren (1:1, kein Decrypt)
    fs::copy(&source_controls, &target_controls)
        .map_err(|e| format!("Kopieren von controls.sii fehlgeschlagen: {}", e))?;

    // #TODO <- Muss ein Show toast noch anzeigen! 

    Ok("controls.sii erfolgreich kopiert".into())
}