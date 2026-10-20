use crate::features::language::translator::t;
use crate::features::profile_clone::logic as profile_clone;
use crate::models::clone_profiles_info::{CloneOptions, CloneTargetStatus};
use std::path::Path;
use tauri::command;

#[command]
pub fn clone_profile_command(
    source_profile: String,
    new_name: String,
    backup: bool,
    replace_hex: bool,
    replace_text: bool,
) -> Result<String, String> {
    let options = CloneOptions {
        backup,
        replace_hex,
        replace_text,
    };

    profile_clone::clone_profile(Path::new(&source_profile), &new_name, options)
        .map(|p| t("ui.clone_success").replace("{path}", &p.display().to_string()))
        .map_err(|e| e.to_string())
}

#[command]
pub fn validate_clone_target(
    source_profile: String,
    new_name: String,
) -> Result<CloneTargetStatus, String> {
    let source = Path::new(&source_profile);

    if !source.exists() {
        return Ok(CloneTargetStatus {
            valid: false,
            message: t("ui.clone_source_missing"),
            target_path: None,
        });
    }

    let parent = source.parent().ok_or_else(|| t("ui.clone_path_invalid"))?;
    let hex_name = crate::shared::hex_float::text_to_hex(&new_name);
    let target_path = parent.join(&hex_name);

    if target_path.exists() {
        return Ok(CloneTargetStatus {
            valid: false,
            message: t("ui.clone_name_taken"),
            target_path: Some(target_path.to_string_lossy().to_string()),
        });
    }

    Ok(CloneTargetStatus {
        valid: true,
        message: t("ui.clone_name_available"),
        target_path: Some(target_path.to_string_lossy().to_string()),
    })
}
