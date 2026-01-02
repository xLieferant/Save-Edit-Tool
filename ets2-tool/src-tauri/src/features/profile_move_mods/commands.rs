use tauri::command;
use std::path::Path;
use std::fs;
use regex::Regex;
use crate::state::AppProfileState;
use crate::shared::current_profile::require_current_profile;
use crate::shared::decrypt::decrypt_if_needed;
use crate::dev_log;

#[command]
pub fn copy_mods_to_profile(
    target_profile_path: String,
    profile_state: tauri::State<'_, AppProfileState>
) -> Result<String, String> {
    // 1. Get current profile path (Source)
    let source_path_str = require_current_profile(profile_state)?;
    let source_path = Path::new(&source_path_str);

    if !source_path.exists() {
        return Err("Quellprofil existiert nicht".into());
    }

    // Check if source and target are the same
    if source_path_str == target_profile_path {
        return Err("Quell- und Zielprofil sind identisch.".into());
    }

    let target_path = Path::new(&target_profile_path);
    if !target_path.exists() {
        return Err("Zielprofil existiert nicht".into());
    }

    dev_log!("Starte Mod-Transfer von '{}' nach '{}'", source_path.display(), target_path.display());

    // 2. Find profile.sii in Source
    let source_sii = source_path.join("profile.sii");
    if !source_sii.exists() {
        return Err("Quell-profile.sii nicht gefunden".into());
    }

    // 3. Decrypt & Read Source
    let source_content = decrypt_if_needed(&source_sii).map_err(|e| e.to_string())?;

    // 4. Extract Mods from Source
    let mods = extract_mods(&source_content);
    dev_log!("Gefundene Mods im Quellprofil: {}", mods.len());

    // 5. Find profile.sii in Target
    let target_sii = target_path.join("profile.sii");
    if !target_sii.exists() {
        return Err("Ziel-profile.sii nicht gefunden".into());
    }

    // 6. Decrypt & Read Target
    let target_content = decrypt_if_needed(&target_sii).map_err(|e| e.to_string())?;

    // 7. Inject Mods into Target Content
    let new_target_content = inject_mods(&target_content, &mods)?;

    // 8. Write back to Target
    fs::write(&target_sii, new_target_content).map_err(|e| format!("Fehler beim Schreiben von profile.sii: {}", e))?;

    dev_log!("Mods erfolgreich übertragen.");
    Ok(format!("Erfolgreich {} Mods übertragen.", mods.len()))
}

fn extract_mods(content: &str) -> Vec<String> {
    // Regex to find lines like: active_mods[0]: "mod_id|mod_name"
    // We capture the content inside the quotes.
    let re = Regex::new(r#"active_mods\[\d+\]:\s*"(.*)""#).unwrap();
    let mut mods = Vec::new();

    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            mods.push(m.as_str().to_string());
        }
    }
    mods
}

fn inject_mods(content: &str, mods: &[String]) -> Result<String, String> {
    let mut new_lines = Vec::new();
    let mut injected = false;
    
    // Regex to identify the count line: active_mods: 123
    let re_count = Regex::new(r"^\s*active_mods:\s*\d+").unwrap();
    // Regex to identify existing mod lines to remove them
    let re_mod_line = Regex::new(r"^\s*active_mods\[\d+\]:").unwrap();

    for line in content.lines() {
        // Skip existing mod lines in target
        if re_mod_line.is_match(line) {
            continue;
        }

        if re_count.is_match(line) {
            // Found the count line. Replace it and append new mods.
            // Preserve indentation from the found line
            let indentation = line.split("active_mods").next().unwrap_or("");
            
            // 1. Write new count
            new_lines.push(format!("{}active_mods: {}", indentation, mods.len()));
            
            // 2. Write new mod lines
            for (i, mod_str) in mods.iter().enumerate() {
                new_lines.push(format!("{}active_mods[{}]: \"{}\"", indentation, i, mod_str));
            }
            injected = true;
        } else {
            new_lines.push(line.to_string());
        }
    }

    if !injected {
        return Err("Konnte 'active_mods' Zeile im Zielprofil nicht finden.".into());
    }

    // Reconstruct file with CRLF
    Ok(new_lines.join("\r\n"))
}