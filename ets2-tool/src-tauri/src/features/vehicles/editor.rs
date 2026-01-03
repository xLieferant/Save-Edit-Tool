use crate::dev_log;
use crate::shared::hex_float::float_to_hex;
use crate::shared::current_profile::{require_current_profile, require_current_save};
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::paths::game_sii_from_save;
use crate::shared::regex_helper::cragex;
use crate::state::AppProfileState;
use regex::{Captures, Regex};
use std::fs;
use std::path::Path;
use tauri::command;

// --------- 
// Helpers
// --------- 

fn read_save_content(profile_state: tauri::State<'_, AppProfileState>) -> Result<(String, String), String> {
    let save_path_str = require_current_save(profile_state.clone()).or_else(|_| {
        let profile = require_current_profile(profile_state)?;
        Ok::<String, String>(format!("{}/save/quicksave", profile))
    })?;
    let path = game_sii_from_save(Path::new(&save_path_str));
    let content = decrypt_if_needed(&path)?;
    Ok((content, path.to_str().unwrap().to_string()))
}

fn write_save_content(path: &str, content: &str) -> Result<(), String> {
    fs::write(path, content.as_bytes()).map_err(|e| e.to_string())
}

fn get_player_vehicle_id(content: &str, vehicle_type: &str) -> Result<String, String> {
    // Fixed: removed unnecessary escaping, { } are not special in character class
    let regex_str = format!(r"player\s*:\s*[A-Za-z0-9._]+\s*\{{\s*[^}}]*?{}\s*:\s*([A-Za-z0-9._]+)", vehicle_type);
    let re = cragex(&regex_str).map_err(|e| format!("Regex Fehler: {}", e))?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| format!("{} nicht gefunden", vehicle_type))
}

// --------------------- 
// Universal Editor
// --------------------- 
fn generic_vehicle_attribute_edit<F>(
    profile_state: tauri::State<'_, AppProfileState>,
    unit_type: &str,      // "vehicle" or "trailer"
    player_vehicle_key: &str, // "my_truck" or "my_trailer"
    attribute_key: &str,
    value_setter: F,
) -> Result<(), String>
where
    F: Fn(&Captures) -> String,
{
    let (content, path) = read_save_content(profile_state)?;
    let vehicle_id = get_player_vehicle_id(&content, player_vehicle_key)?;

    // Fixed: proper brace escaping for format!
    let regex_str = format!(
        r"({}\s*:\s*{}\s*\{{\s*[\s\S]*?{}:\s*)([^\r\n]+)([\s\S]*?\}})",
        unit_type,
        regex::escape(&vehicle_id),
        attribute_key
    );

    let re = Regex::new(&regex_str).map_err(|e| e.to_string())?;
    if !re.is_match(&content) {
        return Err(format!(
            "Attribut '{}' im {}-Block für {} nicht gefunden",
            attribute_key, unit_type, vehicle_id
        ));
    }

    let new_content = re.replace(&content, |caps: &Captures| {
        format!("{}{}{}", &caps[1], value_setter(caps), &caps[3])
    });

    write_save_content(&path, &new_content)
}

// --------------------- 
// Truck Commands
// --------------------- 

#[command]
pub async fn set_player_truck_license_plate(
    plate: String,
    profile_state: tauri::State<'_, AppProfileState>,
) -> Result<(), String> {
    dev_log!("Setting truck license plate to: {}", plate);
    generic_vehicle_attribute_edit(
        profile_state,
        "vehicle",
        "my_truck",
        "license_plate",
        |_| format!(r#""{}""#, plate),
    )
}

#[command]
pub async fn repair_player_truck(profile_state: tauri::State<'_, AppProfileState>) -> Result<(), String> {
    dev_log!("Repairing player truck");
    let (mut content, path) = read_save_content(profile_state)?;
    let truck_id = get_player_vehicle_id(&content, "my_truck")?;

    let wear_attributes = [
        "engine_wear",
        "transmission_wear",
        "cabin_wear",
        "chassis_wear",
    ];

    for attr in &wear_attributes {
        // Fixed: proper brace escaping
        let regex_str = format!(
            r"(vehicle\s*:\s*{}\s*\{{\s*[\s\S]*?{}:\s*)([^ \r\n]+)([\s\S]*?\}})",
            regex::escape(&truck_id),
            attr
        );
        let re = Regex::new(&regex_str).map_err(|e| e.to_string())?;
        if re.is_match(&content) {
            content = re.replace(&content, format!("$1{}$3", float_to_hex(0.0))).to_string();
        }
    }
    
    // Fixed: proper brace escaping
    let re_wheels = Regex::new(&format!(
            r"(vehicle\s*:\s*{}\s*\{{\s*[\s\S]*?wheels_wear\s*:\s*)(\d+)([\s\S]*?\}})",
            regex::escape(&truck_id)
    )).map_err(|e| e.to_string())?;

    if re_wheels.is_match(&content) {
        let mut replacement_done = false;
        content = re_wheels.replace_all(&content, |caps: &Captures| {
            if replacement_done {
                caps[0].to_string()
            } else {
                replacement_done = true;
                let parts = caps[0].split("wear:").collect::<Vec<_>>();
                let mut new_parts = vec![parts[0].to_string()];
                for part in parts.iter().skip(1) {
                    new_parts.push(format!("wear: {}", float_to_hex(0.0)));
                    if let Some(index) = part.find('\n') {
                        new_parts.push(part[index..].to_string());
                    }
                }
                new_parts.join("")
            }
        }).to_string();
    }

    write_save_content(&path, &content)
}

#[command]
pub async fn refuel_player_truck(profile_state: tauri::State<'_, AppProfileState>) -> Result<(), String> {
    dev_log!("Refueling player truck");
    generic_vehicle_attribute_edit(
        profile_state,
        "vehicle",
        "my_truck",
        "fuel_relative",
        |_| float_to_hex(1.0),
    )
}

#[command]
pub async fn set_player_truck_fuel(
    profile_state: tauri::State<'_, AppProfileState>,
    level: f32,
) -> Result<(), String> {
    dev_log!("Set Fuel player truck");
    generic_vehicle_attribute_edit(
        profile_state,
        "vehicle",
        "my_truck",
        "fuel_relative",
        |_| float_to_hex(level),
    )
}

#[command]
pub async fn set_player_truck_wear(
    profile_state: tauri::State<'_, AppProfileState>,
    wear_type: String,
    level: f32,
) -> Result<(), String> {
    dev_log!("Set wear for player truck");
    generic_vehicle_attribute_edit(
        profile_state,
        "vehicle",
        "my_truck",
        &wear_type,
        |_| float_to_hex(level),
    )
}

// ---------------------
// Trailer Commands
// ---------------------

#[command]
pub async fn set_player_trailer_license_plate(
    plate: String,
    profile_state: tauri::State<'_, AppProfileState>,
) -> Result<(), String> {
    dev_log!("Setting trailer license plate to: {}", plate);
    generic_vehicle_attribute_edit(
        profile_state,
        "trailer",
        "my_trailer",
        "license_plate",
        |_| format!(r#""{}""#, plate),
    )
}

#[command]
pub async fn repair_player_trailer(
    profile_state: tauri::State<'_, AppProfileState>,
) -> Result<(), String> {
    dev_log!("Repairing player trailer");
    let (mut content, path) = read_save_content(profile_state)?;
    let trailer_id = get_player_vehicle_id(&content, "my_trailer")?;

    let wear_attributes = ["chassis_wear", "body_wear"];

    for attr in &wear_attributes {
        // Fixed: proper brace escaping
        let regex_str = format!(
            r"(trailer\s*:\s*{}\s*\{{\s*[\s\S]*?{}:\s*)([^ \r\n]+)([\s\S]*?\}})",
            regex::escape(&trailer_id),
            attr
        );
        let re = Regex::new(&regex_str).map_err(|e| e.to_string())?;
        if re.is_match(&content) {
            content = re.replace(&content, format!("$1{}$3", float_to_hex(0.0))).to_string();
        }
    }

    // Fixed: proper brace escaping
    let re_wheels = Regex::new(&format!(
        r"(trailer\s*:\s*{}\s*\{{\s*[\s\S]*?wheels_wear\s*:\s*)(\d+)([\s\S]*?\}})",
        regex::escape(&trailer_id)
    ))
    .map_err(|e| e.to_string())?;

    if re_wheels.is_match(&content) {
        let mut replacement_done = false;
        content = re_wheels
            .replace_all(&content, |caps: &Captures| {
                if replacement_done {
                    caps[0].to_string()
                } else {
                    replacement_done = true;
                    let parts = caps[0].split("wear:").collect::<Vec<_>>();
                    let mut new_parts = vec![parts[0].to_string()];
                    for part in parts.iter().skip(1) {
                        new_parts.push(format!("wear: {}", float_to_hex(0.0)));
                        if let Some(index) = part.find('\n') {
                            new_parts.push(part[index..].to_string());
                        }
                    }
                    new_parts.join("")
                }
            })
            .to_string();
    }

    write_save_content(&path, &content)
}

#[command]
pub async fn set_player_trailer_cargo_mass(
    mass: f32,
    profile_state: tauri::State<'_, AppProfileState>,
) -> Result<(), String> {
    dev_log!("Setting trailer cargo mass to: {}", mass);
    generic_vehicle_attribute_edit(
        profile_state,
        "trailer",
        "my_trailer",
        "cargo_mass",
        |_| float_to_hex(mass),
    )
}