#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::command;
use std::fs;
use serde::Serialize;
use std::process::Command;
use std::path::{Path, PathBuf};
use regex::Regex;

// ---------------------------------------------------------------
// TERMINAL LOG (EINFACHER)
// ---------------------------------------------------------------
macro_rules! log {
    ($($arg:tt)*) => {
        println!($($arg)*)
    };
}

// ---------------------------------------------------------------
// STRUCTS
// ---------------------------------------------------------------

#[derive(Serialize)]
struct ProfileInfo {
    path: String,
    name: Option<String>,
    success: bool,
    message: Option<String>,
}

// Profilname extrahieren aus profile.sii
fn extract_profile_name(text: &str) -> Option<String> {
    let re = Regex::new(r#"(?i)^\s*profile_name\s*:\s*"([^"]+)""#).unwrap();
    for l in text.lines() {
        if let Some(c) = re.captures(l) {
            return Some(c[1].to_string());
        }
    }
    None
}

// ---------------------------------------------------------------
// 0) SII Datei vorher entschlüsseln (NEU!)
// ---------------------------------------------------------------

fn ensure_decrypted(path: &PathBuf) -> Result<(), String> {
    let content = fs::read_to_string(path).unwrap_or_default();

    // Bereits Klartext?
    if content.starts_with("SiiNunit") {
        log!("Bereits Klartext: {}", path.display());
        return Ok(());
    }

    log!("Decrypting: {}", path.display());

    let exe = PathBuf::from("tools/SII_Decrypt.exe");
    if !exe.exists() {
        return Err("SII_Decrypt.exe nicht gefunden".into());
    }

    let out = Command::new(exe)
        .arg(path)
        .output()
        .map_err(|e| format!("Fehler beim Ausführen: {}", e))?;

    if !out.status.success() {
        return Err(format!(
            "Decrypt-Fehler: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------
// 1) PROFILE FINDEN
// ---------------------------------------------------------------

#[command]
fn find_ets2_profiles() -> Vec<ProfileInfo> {
    log!("Starte Profil-Suche…");

    let mut found_profiles = Vec::new();

    if let Some(documents) = dirs::document_dir() {
        let base = documents.join("Euro Truck Simulator 2");
        let folders = vec![
            base.join("profiles"),
            base.join("profiles.backup"),
            base.join("steam_profiles"),
            base.clone(),
        ];

        for folder in folders {
            if !folder.exists() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(folder) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let sii = path.join("profile.sii");

                    if !(path.is_dir() && sii.exists()) {
                        continue;
                    }

                    // ERSTER SCHRITT → immer decrypt versuchen
                    let _ = ensure_decrypted(&sii);

                    let text = fs::read_to_string(&sii).ok();

                    let mut info = ProfileInfo {
                        path: path.display().to_string(),
                        name: None,
                        success: false,
                        message: None,
                    };

                    if let Some(content) = text {
                        if let Some(name) = extract_profile_name(&content) {
                            info.name = Some(name);
                            info.success = true;
                            info.message = Some("OK".into());
                            log!("Profil gefunden: {}", info.path);
                        } else {
                            info.message = Some("profile_name nicht gefunden".into());
                        }
                    } else {
                        info.message = Some("profile.sii nicht lesbar".into());
                    }

                    found_profiles.push(info);
                }
            }
        }
    }

    log!("Profil-Suche abgeschlossen.");
    found_profiles
}

// ---------------------------------------------------------------
// 2) Autosave Pfad generieren
// ---------------------------------------------------------------

fn autosave_path(profile_path: &str) -> PathBuf {
    Path::new(profile_path)
        .join("save")
        .join("autosave")
        .join("info.sii")
}

// ---------------------------------------------------------------
// 3) SII decrypt + Fallback für autosave
// ---------------------------------------------------------------

fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    log!("decrypt_if_needed: {}", path.display());

    let out = std::env::temp_dir()
        .join("ets2_tool")
        .join("decoded_autosave.sii");

    let _ = fs::create_dir_all(out.parent().unwrap());

    let decrypt = Command::new("tools/SII_Decrypt.exe")
        .arg(path)
        .arg(&out)
        .output();

    match decrypt {
        Ok(output) => {
            if output.status.success() {
                log!("Decrypt erfolgreich.");
                return fs::read_to_string(&out)
                    .map_err(|e| format!("Fehler beim Lesen entschlüsselter Datei: {}", e));
            } else {
                log!("Decrypt fehlgeschlagen, lese Klartext.");
            }
        }
        Err(e) => log!("Decrypt nicht ausführbar: {}", e),
    }

    fs::read_to_string(path).map_err(|e| format!("Fehler beim Lesen: {}", e))
}

// ---------------------------------------------------------------
// 4) Profil laden & CURRENT_PROFILE setzen
// ---------------------------------------------------------------

#[command]
fn load_profile(profile_path: String) -> Result<String, String> {
    log!("load_profile gestartet: {}", &profile_path);

    let autosave = autosave_path(&profile_path);

    if !autosave.exists() {
        let msg = format!("Autosave nicht gefunden: {}", autosave.display());
        log!("FEHLER: {}", msg);
        return Err(msg);
    }

    std::env::set_var("CURRENT_PROFILE", &profile_path);

    log!("Profil erfolgreich geladen.");
    Ok(format!("Profil geladen. Autosave: {}", autosave.display()))
}

// ---------------------------------------------------------------
// 5) Geld lesen
// ---------------------------------------------------------------

#[command]
fn read_money() -> Result<i64, String> {
    log!("read_money gestartet");

    let profile = std::env::var("CURRENT_PROFILE")
        .map_err(|_| "Kein Profil geladen. load_profile zuerst aufrufen.".to_string())?;

    let path = autosave_path(&profile);

    let content = decrypt_if_needed(&path)?;

    let re = Regex::new(r"info_money_account:\s*(\d+)").unwrap();

    if let Some(c) = re.captures(&content) {
        let money = c[1].parse::<i64>().unwrap_or(0);
        log!("Geld gefunden: {}", money);
        return Ok(money);
    }

    Err("Geldwert nicht gefunden".into())
}

// ---------------------------------------------------------------
// 6) Geld ändern
// ---------------------------------------------------------------

#[command]
fn edit_money(amount: i64) -> Result<(), String> {
    log!("edit_money: {}", amount);

    let profile = std::env::var("CURRENT_PROFILE")
        .map_err(|_| "Kein Profil geladen.".to_string())?;

    let path = autosave_path(&profile);

    let content = decrypt_if_needed(&path)?;

    let re = Regex::new(r"info_money_account:\s*\d+").unwrap();
    let new = re.replace(&content, format!("info_money_account: {}", amount));

    fs::write(&path, new.as_bytes()).map_err(|e| e.to_string())?;

    log!("Geld erfolgreich geändert.");
    Ok(())
}

// ---------------------------------------------------------------
// 7) Level ändern
// ---------------------------------------------------------------

#[command]
fn edit_level(xp: i64) -> Result<(), String> {
    log!("edit_level: {}", xp);

    let profile = std::env::var("CURRENT_PROFILE")
        .map_err(|_| "Kein Profil geladen.".to_string())?;

    let path = autosave_path(&profile);

    let content = decrypt_if_needed(&path)?;

    let re = Regex::new(r"info_players_experience:\s*\d+").unwrap();
    let new = re.replace(&content, format!("info_players_experience: {}", xp));

    fs::write(&path, new.as_bytes()).map_err(|e| e.to_string())?;

    log!("Level erfolgreich geändert.");
    Ok(())
}

// ---------------------------------------------------------------
// Tauri Start
// ---------------------------------------------------------------

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            find_ets2_profiles,
            load_profile,
            read_money,
            edit_money,
            edit_level
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
