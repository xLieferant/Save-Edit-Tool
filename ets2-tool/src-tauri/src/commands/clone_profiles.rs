use tauri::command;
use std::path::Path;
use crate::utils::profile_clone;
use crate::models::clone_profiles_info::{CloneOptions, CloneTargetStatus};


#[command]
pub fn clone_profile_command(
    source_profile: String,
    new_name: String,
    backup: bool,
    replace_hex: bool,
    replace_text: bool
) -> Result<String, String> {
    let options = CloneOptions {
        backup,
        replace_hex,
        replace_text,
    };

    profile_clone::clone_profile(Path::new(&source_profile), &new_name, options)
        .map(|p| format!("Profil erfolgreich geklont nach: {}", p.display()))
        .map_err(|e| e.to_string())
}

#[command]
pub fn validate_clone_target(
    source_profile: String,
    new_name: String
) -> Result<CloneTargetStatus, String> {
    let source = Path::new(&source_profile);
    
    if !source.exists() {
        return Ok(CloneTargetStatus {
            valid: false,
            message: "Quellprofil existiert nicht.".into(),
            target_path: None,
        });
    }

    let parent = source.parent().ok_or("Konnte Elternverzeichnis nicht finden")?;
    // Hinweis: Hier wird new_name direkt als Ordnername verwendet. 
    // Falls du Hex-Ordnernamen willst, müsste man new_name hier noch hex-encoden.
    let target_path = parent.join(&new_name);

    if target_path.exists() {
        return Ok(CloneTargetStatus {
            valid: false,
            message: format!("Ein Profilordner mit dem Namen '{}' existiert bereits.", new_name),
            target_path: Some(target_path.to_string_lossy().to_string()),
        });
    }

    Ok(CloneTargetStatus {
        valid: true,
        message: "Name verfügbar.".into(),
        target_path: Some(target_path.to_string_lossy().to_string()),
    })
}
