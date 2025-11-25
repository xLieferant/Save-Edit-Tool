#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::command;
use std::process::Command;
use std::fs;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct ProfileInfo {
    path: String,
    name: Option<String>,   // Profilname, falls gefunden
    success: bool,          // true = SII_decrypt hat funktioniert
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
            if folder.exists() {
                if let Ok(entries) = fs::read_dir(&folder) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let sii_file = path.join("profile.sii");

                        if path.is_dir() && sii_file.exists() {
                            let mut info = ProfileInfo {
                                path: path.display().to_string(),
                                name: None,
                                success: false,
                            };

                            // Temporäre Datei im selben Ordner
                            let tmp_file: PathBuf = path.join("profile_decoded.sii");

                            // Schritt 1: Tool ausführen -> schreibt in tmp_file
                            let result = Command::new("src-tauri/tools/SII_decrypt.exe")
                                .arg(&sii_file)
                                .arg(&tmp_file) // viele Tools akzeptieren Eingabe + Ausgabe
                                .status();

                            if let Ok(status) = result {
                                if status.success() {
                                    // Schritt 2: temporäre Datei lesen
                                    if let Ok(content) = fs::read_to_string(&tmp_file) {
                                        if let Some(line) = content.lines().find(|l| l.contains("Profile_name")) {
                                            if let Some(start) = line.find('"') {
                                                if let Some(end) = line.rfind('"') {
                                                    info.name = Some(line[start+1..end].to_string());
                                                    info.success = true;
                                                }
                                            }
                                        }
                                    }
                                    // Schritt 3: temporäre Datei wieder löschen
                                    let _ = fs::remove_file(&tmp_file);
                                }
                            }

                            found.push(info);
                        }
                    }
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
