#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::command;
use std::fs;
use serde::Serialize;
use std::process::Command;
use std::path::{Path, PathBuf};
use regex::Regex;

#[derive(Serialize)]
struct ProfileInfo {
    path: String,
    name: Option<String>,
    success: bool,
    message: Option<String>,
}

fn extract_profile_name(text: &str) -> Option<String> {
    let re = Regex::new(r#"(?i)^\s*profile_name\s*:\s*"([^"]+)""#).unwrap();
    for l in text.lines() {
        if let Some(c) = re.captures(l) {
            return Some(c[1].to_string());
        }
    }
    None
}

// ---------- PROFILE FINDEN --------------------------------------------------

#[command]
fn find_ets2_profiles() -> Vec<ProfileInfo> {
    let mut found = Vec::new();

    if let Some(documents) = dirs::document_dir() {
        let base = documents.join("Euro Truck Simulator 2");
        let folders = vec![
            base.join("profiles"),
            base.join("profiles.backup"),
            base.join("steam_profiles"),
            base.clone(),
        ];

        for folder in folders {
            if !folder.exists() { continue; }

            if let Ok(entries) = fs::read_dir(folder) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let sii_file = path.join("profile.sii");

                    if !(path.is_dir() && sii_file.exists()) {
                        continue;
                    }

                    let mut info = ProfileInfo {
                        path: path.display().to_string(),
                        name: None,
                        success: false,
                        message: None,
                    };

                    let tmp_out = std::env::temp_dir()
                        .join("ets2_tool")
                        .join(format!("{}_decoded.sii",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ));

                    let _ = fs::create_dir_all(tmp_out.parent().unwrap());

                    // Versuch 1: decrypt
                    let decrypt_res = Command::new("tools/SII_decrypt.exe")
                        .arg(&sii_file)
                        .arg(&tmp_out)
                        .status();

                    let content = match decrypt_res {
                        Ok(s) if s.success() => fs::read_to_string(&tmp_out).ok(),
                        _ => fs::read_to_string(&sii_file).ok(),
                    };

                    if let Some(text) = content {
                        if let Some(name) = extract_profile_name(&text) {
                            info.name = Some(name);
                            info.success = true;
                            info.message = Some("OK".into());
                        } else {
                            info.message = Some("profile_name nicht gefunden".into());
                        }
                    } else {
                        info.message = Some("Datei nicht lesbar".into());
                    }

                    let _ = fs::remove_file(tmp_out);

                    found.push(info);
                }
            }
        }
    }

    found
}

// -----------------------------------------------------------------------------
// ---------- PROFILE LADEN & AUTOSAVE BEARBEITEN -------------------------------
// -----------------------------------------------------------------------------

fn autosave_path(profile: &str) -> PathBuf {
    Path::new(profile)
        .join("save")
        .join("autosave")
        .join("info.sii")
}

fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    let out_file = std::env::temp_dir()
        .join("ets2_tool")
        .join("autosave_decoded.sii");

    let _ = fs::create_dir_all(out_file.parent().unwrap());

    let status = Command::new("tools/SII_decrypt.exe")
        .arg(path)
        .arg(&out_file)
        .status();

    if let Ok(s) = status {
        if s.success() {
            return fs::read_to_string(&out_file)
                .map_err(|e| e.to_string());
        }
    }

    // fallback → Klartext lesen
    fs::read_to_string(path).map_err(|e| e.to_string())
}

fn write_back(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())
}

#[command]
fn load_profile(path: String) -> Result<String, String> {
    let info_path = autosave_path(&path);

    if !info_path.exists() {
        return Err("autosave/info.sii nicht gefunden!".into());
    }

    Ok(format!("Profil geladen: {}", info_path.display()))
}

#[command]
fn read_money() -> Result<i32, String> {
    let profile_path = std::env::var("CURRENT_PROFILE")
        .map_err(|_| "Kein Profil gesetzt".to_string())?;

    let info_path = autosave_path(&profile_path);

    let content = decrypt_if_needed(&info_path)?;

    let re = Regex::new(r#"info_money_account:\s*(\d+)"#).unwrap();

    if let Some(caps) = re.captures(&content) {
        let value = caps[1].parse::<i32>().unwrap_or(0);
        println!("Aktueller Geldwert: {}", value);
        return Ok(value);
    }

    Err("Kein Geld-Wert gefunden".into())
}


#[command]
fn edit_money(amount: i32) -> Result<(), String> {
    let profile = std::env::var("CURRENT_PROFILE").unwrap_or_default();
    let path = autosave_path(&profile);

    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(r#"info_money_account:\s*\d+"#).unwrap();

    let new = re.replace(&content, format!("info_money_account: {}", amount));
    write_back(&path, &new)?;

    Ok(())
}

#[command]
fn edit_level(level: i32) -> Result<(), String> {
    let profile = std::env::var("CURRENT_PROFILE").unwrap_or_default();
    let path = autosave_path(&profile);

    let content = decrypt_if_needed(&path)?;
    let re = Regex::new(r#"info_players_experience:\s*\d+"#).unwrap();

    let new = re.replace(&content, format!("info_players_experience: {}", level));
    write_back(&path, &new)?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            find_ets2_profiles,
            load_profile,
            edit_money,
            edit_level
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
