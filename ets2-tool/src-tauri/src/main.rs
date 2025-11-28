#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use regex::Regex;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::command;

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

#[derive(Serialize)]
struct SaveGameData {
    money: Option<i64>,
    xp: Option<i64>,
    level: Option<i64>,
    garages: Option<i64>,
    trucks_owned: Option<i64>,
    trailers_owned: Option<i64>,
    kilometers_total: Option<i64>,
}

// ---------------------------------------------------------------
// Flexible Profile-Name Extraktion
// erkennt:
//   profile_name: "Name"
//   profile_name: Name
//   PROFILE_NAME : "Name"
// ---------------------------------------------------------------
fn extract_profile_name(text: &str) -> Option<String> {
    // flexible regex, erlaubt optional quotes
    let re = Regex::new(r#"(?i)profile_name\s*:\s*"?(?P<name>[^"\r\n]+)"?"#).unwrap();
    if let Some(cap) = re.captures(text) {
        return Some(cap[1].trim().to_string());
    }
    None
}

// ---------------------------------------------------------------
// HEX-Ordnername -> UTF-8 dekodieren (ETS2 verwendet hex-codierte ordner)
// z.B. 547275636B... -> "Truck..." etc.
// ---------------------------------------------------------------
fn decode_hex_folder_name(hex: &str) -> Option<String> {
    // entferne mögliche nicht-hexzeichen & whitespace
    let hex_clean: String = hex.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex_clean.len() % 2 != 0 || hex_clean.is_empty() {
        return None;
    }

    let bytes_res: Result<Vec<u8>, _> = (0..hex_clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_clean[i..i + 2], 16))
        .collect();

    match bytes_res {
        Ok(bytes) => String::from_utf8(bytes).ok(),
        Err(_) => None,
    }
}

// ---------------------------------------------------------------
// 0) SII Datei vorher entschlüsseln (prüft, ob SiiNunit oder nicht)
// decrypt_if_needed versucht, SII_Decrypt.exe (oder decrypt_truck) zu nutzen
// und liest die temporäre Ausgabedatei. Falls das nicht klappt, liest es die Originaldatei.
// ---------------------------------------------------------------
fn decrypt_if_needed(path: &Path) -> Result<String, String> {
    log!("decrypt_if_needed: {}", path.display());

    // Versuch: lies original kurz, um zu prüfen, ob bereits Klartext
    if let Ok(orig) = fs::read_to_string(path) {
        if orig.starts_with("SiiNunit") {
            // bereits Klartext, gib zurück
            return Ok(orig);
        }
    }

    // ansonsten versuche externes Tool: erst SII_Decrypt.exe im tools/, dann global decrypt_truck
    let out = std::env::temp_dir()
        .join("ets2_tool")
        .join(format!("decoded_{}.sii", path.file_stem().unwrap_or_else(|| std::ffi::OsStr::new("tmp")).to_string_lossy()));

    let _ = fs::create_dir_all(out.parent().unwrap());

    // prefer local tools/SII_Decrypt.exe if exists
    let try_local = PathBuf::from("tools/SII_Decrypt.exe");
    let mut did_decrypt = false;

    if try_local.exists() {
        log!("Versuche tools/SII_Decrypt.exe");
        if let Ok(output) = Command::new(&try_local).arg(path).arg(&out).output() {
            if output.status.success() {
                did_decrypt = true;
            } else {
                log!("tools/SII_Decrypt.exe fehlgeschlagen: {}", String::from_utf8_lossy(&output.stderr));
            }
        } else {
            log!("tools/SII_Decrypt.exe konnte nicht ausgeführt werden");
        }
    }

    // fallback: global decrypt_truck (wenn im PATH oder in cargo bin installiert)
    if !did_decrypt {
        if let Ok(output) = Command::new("decrypt_truck").arg(path).arg(&out).output() {
            if output.status.success() {
                did_decrypt = true;
            } else {
                log!("decrypt_truck fehlgeschlagen: {}", String::from_utf8_lossy(&output.stderr));
            }
        } else {
            log!("decrypt_truck nicht ausführbar (nicht im PATH?)");
        }
    }

    // wenn ein Tool die entschlüsselte Datei geschrieben hat, lese sie
    if did_decrypt && out.exists() {
        return fs::read_to_string(&out).map_err(|e| format!("Fehler beim Lesen der entschlüsselten Datei: {}", e));
    }

    // letzter Fallback: versuche die Originaldatei als Text (kann verschlüsselt sein)
    fs::read_to_string(path).map_err(|e| format!("Fehler beim Lesen der Originaldatei: {}", e))
}

// ---------------------------------------------------------------
// 1) PROFILE FINDEN (mit Decrypt + Fallback-Namen aus profile.bak.sii oder HEX-Ordner)
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
            base.clone(),
        ];

        for folder in folders {
            if !folder.exists() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(folder) {
                for entry in entries.flatten() {
                    let path = entry.path();

                    // skip if not directory
                    if !path.is_dir() {
                        continue;
                    }

                    let sii = path.join("profile.sii");
                    if !sii.exists() {
                        continue;
                    }

                    // Versuche zu decrypten UND direkt den Klartext einzulesen.
                    let text = match decrypt_if_needed(&sii) {
                        Ok(content) => Some(content),
                        Err(e) => {
                            log!(
                                "Decrypt fehlgeschlagen für {}: {}. Versuche Klartext-Fallback.",
                                sii.display(),
                                e
                            );
                            fs::read_to_string(&sii).ok()
                        }
                    };

                    let mut info = ProfileInfo {
                        path: path.display().to_string(),
                        name: None,
                        success: false,
                        message: None,
                    };

                    // 1) profilname aus profile.sii
                    let from_sii = text.as_ref().and_then(|t| extract_profile_name(t));

                    // 2) falls leer -> versuche profile.bak.sii
                    let from_bak = if from_sii.is_none() {
                        let bak = path.join("profile.bak.sii");
                        if bak.exists() {
                            match decrypt_if_needed(&bak) {
                                Ok(bak_content) => extract_profile_name(&bak_content),
                                Err(_) => fs::read_to_string(&bak).ok().and_then(|t| extract_profile_name(&t)),
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // 3) falls noch leer -> dekodiere ordnername hex -> text
                    let from_folder = if from_sii.is_none() && from_bak.is_none() {
                        path.file_name()
                            .and_then(|os| os.to_str())
                            .and_then(|s| decode_hex_folder_name(s))
                    } else {
                        None
                    };

                    // priorität: sii > bak > folder
                    if let Some(name) = from_sii.or(from_bak).or(from_folder) {
                        info.name = Some(name);
                        info.success = true;
                        info.message = Some("OK".into());
                        log!("Profil gefunden: {} ({})", info.path, info.name.as_ref().unwrap());
                    } else {
                        info.message = Some("profile_name nicht gefunden".into());
                        log!("profile_name nicht gefunden in {}", info.path);
                    }

                    found_profiles.push(info);
                }
            }
        }
    }

    log!("Profil-Suche abgeschlossen. Gefunden: {}", found_profiles.len());
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
// 3) decrypt_if_needed für autosave (wiederverwendbar)
// ---------------------------------------------------------------
// Note: diese Funktion wurde oben bereits implementiert (gleiches Verhalten),
// hier als wrapper nur um Signatur für andere Teile klar zu machen.
#[allow(dead_code)]
fn decrypt_if_needed_wrapper(path: &Path) -> Result<String, String> {
    decrypt_if_needed(path)
}

// ---------------------------------------------------------------
// 4) Profil laden
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

    let profile =
        std::env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;

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
// 5.1) XP lesen
// ---------------------------------------------------------------
#[command]
fn read_xp() -> Result<i64, String> {
    log!("read_xp gestartet");

    let profile =
        std::env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;

    let path = autosave_path(&profile);

    let content = decrypt_if_needed(&path)?;

    let re = Regex::new(r"info_players_experience:\s*(\d+)").unwrap();

    if let Some(c) = re.captures(&content) {
        let xp = c[1].parse::<i64>().unwrap_or(0);
        log!("XP gefunden: {}", xp);
        return Ok(xp);
    }

    Err("XP nicht gefunden".into())
}

// ---------------------------------------------------------------
// 6) Geld ändern
// ---------------------------------------------------------------
#[command]
fn edit_money(amount: i64) -> Result<(), String> {
    log!("edit_money: {}", amount);

    let profile =
        std::env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;

    let path = autosave_path(&profile);

    let content = decrypt_if_needed(&path)?;

    let re = Regex::new(r"info_money_account:\s*\d+").unwrap();
    let new = re.replace(&content, format!("info_money_account: {}", amount));

    fs::write(&path, new.as_bytes()).map_err(|e| e.to_string())?;

    log!("Geld geändert.");
    Ok(())
}

// ---------------------------------------------------------------
// 7) XP ändern
// ---------------------------------------------------------------
#[command]
fn edit_level(xp: i64) -> Result<(), String> {
    log!("edit_level: {}", xp);

    let profile =
        std::env::var("CURRENT_PROFILE").map_err(|_| "Kein Profil geladen.".to_string())?;

    let path = autosave_path(&profile);

    let content = decrypt_if_needed(&path)?;

    let re = Regex::new(r"info_players_experience:\s*\d+").unwrap();
    let new = re.replace(&content, format!("info_players_experience: {}", xp));

    fs::write(&path, new.as_bytes()).map_err(|e| e.to_string())?;

    log!("Level geändert.");
    Ok(())
}

// ---------------------------------------------------------------
// 8) ALLE Daten auf einmal auslesen
// ---------------------------------------------------------------
#[command]
fn read_all_profile_data() -> Result<SaveGameData, String> {
    log!("read_all_profile_data gestartet");

    let profile = std::env::var("CURRENT_PROFILE")
        .map_err(|_| "Kein Profil geladen.".to_string())?;

    let path = autosave_path(&profile);

    let content = decrypt_if_needed(&path)?;

    let money_re = Regex::new(r"info_money_account:\s*(\d+)").unwrap();
    let xp_re = Regex::new(r"info_players_experience:\s*(\d+)").unwrap();
    let level_re = Regex::new(r"info_player_level:\s*(\d+)").unwrap();
    let garages_re = Regex::new(r"garages:\s*(\d+)").unwrap();
    let trucks_re = Regex::new(r"trucks:\s*(\d+)").unwrap();
    let trailers_re = Regex::new(r"trailers:\s*(\d+)").unwrap();
    let km_re = Regex::new(r"km_total:\s*(\d+)").unwrap();

    let data = SaveGameData {
        money: money_re.captures(&content).and_then(|c| c[1].parse().ok()),
        xp: xp_re.captures(&content).and_then(|c| c[1].parse().ok()),
        level: level_re.captures(&content).and_then(|c| c[1].parse().ok()),
        garages: garages_re
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        trucks_owned: trucks_re.captures(&content).and_then(|c| c[1].parse().ok()),
        trailers_owned: trailers_re
            .captures(&content)
            .and_then(|c| c[1].parse().ok()),
        kilometers_total: km_re.captures(&content).and_then(|c| c[1].parse().ok()),
    };

    log!("Alle Savegame Daten geladen.");
    Ok(data)
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
            read_xp,
            edit_money,
            edit_level,
            read_all_profile_data
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
