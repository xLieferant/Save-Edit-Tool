#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::command;
use std::fs;
use std::path::PathBuf;

#[command]
fn find_ets2_profiles() -> Vec<String> {
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
                        if path.is_dir() && path.join("profile.sii").exists() {
                            found.push(path.display().to_string());
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
