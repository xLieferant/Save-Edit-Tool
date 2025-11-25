#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::command;
use std::process::Command;
use std::fs;
use serde::Serialize;
use std::path::PathBuf;
use regex::Regex;

#[derive(Serialize)]
struct ProfileInfo {
    path: String,
    name: Option<String>,   // Profilname, falls gefunden
    success: bool,          // true = SII_decrypt hat funktioniert
    message: Option<String>, // Fehler Statushinweis
}

fn extract_profile_name(text: &str) -> Option<String> {
    // Sucht profile_name: "…", tolerant bei Whitespaces, Case-insensitive
    let re = Regex::new(r#"(?i)^\s*profile_name\s*:\s*"([^"]+)""#).unwrap();
    for line in text.lines() {
        if let Some(caps) = re.captures(line) {
            return Some(caps[1].to_string());
        }
    }
    None
}

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

            if let Ok(entries) = fs::read_dir(&folder) {
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

                    // Temporäre Ausgabedatei im Temp-Ordner (sicher, keine Savegame-Änderung)
                    let tmp_out: PathBuf = std::env::temp_dir()
                        .join("ets2_tool")
                        .join(format!("{}_profile_decoded.sii",
                                      path.file_name().unwrap_or_default().to_string_lossy()));

                    // Stelle sicher, dass der Zielordner existiert
                    if let Err(e) = fs::create_dir_all(tmp_out.parent().unwrap()) {
                        info.message = Some(format!("Temp-Verzeichnis konnte nicht erstellt werden: {e}"));
                        found.push(info);
                        continue;
                    }

                    // 1) SII_decrypt.exe ausführen
                    // Falls dein Tool nur Input akzeptiert und Output in dieselbe Datei schreibt,
                    // dann ersetze den .arg(&tmp_out) unten durch .arg("--output").arg(&tmp_out) o.ä.,
                    // je nach CLI deines Tools.
                    let status = Command::new("tools/SII_decrypt.exe")
                        .arg(&sii_file)    // Eingabe
                        .arg(&tmp_out)     // Ausgabe (wenn dein Tool das so erwartet)
                        .status();

                    match status {
                        Ok(s) if s.success() => {
                            // 2) Entschlüsselte Datei lesen
                            match fs::read_to_string(&tmp_out) {
                                Ok(content) => {
                                    if let Some(name) = extract_profile_name(&content) {
                                        info.name = Some(name);
                                        info.success = true;
                                        info.message = Some("OK".into());
                                    } else {
                                        info.message = Some("profile_name nicht gefunden".into());
                                    }
                                }
                                Err(e) => {
                                    info.message = Some(format!("Temp-Datei nicht lesbar: {e}"));
                                }
                            }
                        }
                        Ok(s) => {
                            info.message = Some(format!("SII_decrypt beendet mit Status {}", s));
                        }
                        Err(e) => {
                            info.message = Some(format!("SII_decrypt nicht ausführbar: {e}"));
                        }
                    }

                    // 3) Temp-Datei aufräumen
                    let _ = fs::remove_file(&tmp_out);

                    found.push(info);
                }
            }
        }
    }

    found
}

#[command]
fn load_profile(path: String) -> Result<String, String> {
    if std::path::Path::new(&path).exists() {
        Ok(format!("Profil geladen: {}", path))
    } else {
        Err("Profil konnte nicht geladen werden".into())
    }
}

#[command]
fn edit_money(amount: i32) -> Result<(), String> {
    println!("Geld setzen: {}", amount);
    Ok(())
}

#[command]
fn edit_level(level: i32) -> Result<(), String> {
    println!("Level setzen: {}", level);
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
        .expect("error while running tauri application");
}
