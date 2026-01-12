use crate::dev_log;
use crate::shared::hex_float::float_to_hex;
use crate::shared::current_profile::{require_current_profile, require_current_save};
use crate::shared::decrypt::decrypt_cached;
use crate::shared::paths::game_sii_from_save;
use crate::shared::regex_helper::cragex;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use regex::{Captures, Regex};
use std::fs;
use std::path::Path;
use tauri::command;

// --------- 
// Helpers
// --------- 

fn read_save_content(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
) -> Result<(String, String), String> {
    let save_path_str = require_current_save(profile_state.clone()).or_else(|_| {
        let profile = require_current_profile(profile_state)?;
        Ok::<String, String>(format!("{}/save/quicksave", profile))
    })?;
    let path = game_sii_from_save(Path::new(&save_path_str));
    let content = decrypt_cached(&path, &decrypt_cache)?;
    Ok((content, path.to_str().unwrap().to_string()))
}

fn write_save_content(path: &str, content: &str) -> Result<(), String> {
    fs::write(path, content.as_bytes()).map_err(|e| e.to_string())
}

fn get_player_vehicle_id(content: &str, vehicle_type: &str) -> Result<String, String> {
    let regex_str = format!(r"player\s*:\s*[A-Za-z0-9._]+\s*\{{\s*[^}}]*?{}\s*:\s*([A-Za-z0-9._]+)", vehicle_type);
    let re = cragex(&regex_str).map_err(|e| format!("Regex Fehler: {}", e))?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| format!("{} nicht gefunden", vehicle_type))
}

// ← NEW: Extract complete vehicle/trailer block with proper brace matching
fn extract_vehicle_block(content: &str, block_type: &str, vehicle_id: &str) -> Result<(usize, usize), String> {
    let start_pattern = format!(r"{}\s*:\s*{}\s*\{{", block_type, regex::escape(vehicle_id));
    let re_start = Regex::new(&start_pattern).map_err(|e| e.to_string())?;
    
    let cap = re_start.captures(content)
        .ok_or(format!("{} block for {} not found", block_type, vehicle_id))?;
    
    let start_pos = cap.get(0).unwrap().end();
    
    // Count braces to find the matching closing brace
    let mut brace_count = 1;
    let mut end_pos = start_pos;
    let chars: Vec<char> = content[start_pos..].chars().collect();
    
    for (i, ch) in chars.iter().enumerate() {
        if *ch == '{' {
            brace_count += 1;
        } else if *ch == '}' {
            brace_count -= 1;
            if brace_count == 0 {
                end_pos = start_pos + i;
                break;
            }
        }
    }
    
    if brace_count != 0 {
        return Err(format!("Unmatched braces in {} block", block_type));
    }
    
    // Return positions INCLUDING the opening brace position
    Ok((cap.get(0).unwrap().start(), end_pos + 1))
}

// #[x] : Function needs to find and delete something inside the regex 
// and at the end, it should look like this inside the game.sii; license_plate; "newNameID|countryID" (countryID is set, automatically, we're not deleting this info)
// --------------------- 
// Universal Editor
// --------------------- 
fn generic_vehicle_attribute_edit<F>(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
    unit_type: &str,      // "vehicle" or "trailer"
    player_vehicle_key: &str, // "my_truck" or "my_trailer"
    attribute_key: &str,
    value_setter: F,
) -> Result<(), String>
where
    F: Fn(&Captures) -> String,
{
    let (content, path) = read_save_content(profile_state, decrypt_cache.clone())?;
    let vehicle_id = get_player_vehicle_id(&content, player_vehicle_key)?;

    // ← CHANGED: Use proper brace matching
    let (block_start, block_end) = extract_vehicle_block(&content, unit_type, &vehicle_id)?;
    let block = &content[block_start..block_end];
    
    // Search for attribute within this specific block
    let regex_str = format!(r"({}:\s*)([^\r\n]+)", attribute_key);
    let re = Regex::new(&regex_str).map_err(|e| e.to_string())?;
    
    if !re.is_match(block) {
        return Err(format!(
            "Attribut '{}' im {}-Block für {} nicht gefunden",
            attribute_key, unit_type, vehicle_id
        ));
    }

    let new_block = re.replace(block, |caps: &Captures| {
        format!("{}{}", &caps[1], value_setter(caps))
    });

    let new_content = format!("{}{}{}", &content[..block_start], new_block, &content[block_end..]);
    write_save_content(&path, &new_content)?;

    decrypt_cache.invalidate_path(Path::new(&path));
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();

    Ok(())
}

// --------------------- 
// Truck Commands
// --------------------- 

#[command]
pub async fn set_player_truck_license_plate(
    plate: String,
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
) -> Result<(), String> {
    dev_log!("Setting truck license plate to: {}", plate);
    generic_vehicle_attribute_edit(
        profile_state,
        decrypt_cache,
        profile_cache,
        "vehicle",
        "my_truck",
        "license_plate",
        |caps: &Captures| {
            let old_value = &caps[2];
            let old_value_unquoted = old_value.trim_matches('"');
            if let Some(pipe_index) = old_value_unquoted.rfind('|') {
                let country_part = &old_value_unquoted[pipe_index + 1..];
                format!(r#""{}|{}""#, &plate, country_part)
            } else {
                format!(r#""{}""#, &plate)
            }
        },
    )
}

#[command]
pub async fn repair_player_truck(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
) -> Result<(), String> {
    dev_log!("Repairing player truck");
    let (content, path) = read_save_content(profile_state, decrypt_cache.clone())?;
    let truck_id = get_player_vehicle_id(&content, "my_truck")?;

    // ← CHANGED: Use proper brace matching
    let (block_start, block_end) = extract_vehicle_block(&content, "vehicle", &truck_id)?;
    let mut block = content[block_start..block_end].to_string();

    let wear_attributes = [
        "engine_wear",
        "transmission_wear",
        "cabin_wear",
        "chassis_wear",
    ];

    for attr in &wear_attributes {
        let regex_str = format!(r"({}:\s*)([^ \r\n]+)", attr);
        let re = Regex::new(&regex_str).map_err(|e| e.to_string())?;
        if re.is_match(&block) {
            block = re.replace(&block, format!("$1{}", float_to_hex(0.0))).to_string();
        }
    }
    
    // Fix wheels_wear array
    let re_wheels = Regex::new(r"wheels_wear\[\d+\]:\s*[^ \r\n]+").unwrap();
    block = re_wheels.replace_all(&block, |_: &Captures| {
        format!("wheels_wear[0]: {}", float_to_hex(0.0))
    }).to_string();

    let new_content = format!("{}{}{}", &content[..block_start], block, &content[block_end..]);
    write_save_content(&path, &new_content)?;

    decrypt_cache.invalidate_path(Path::new(&path));
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();

    Ok(())
}

#[command]
pub async fn refuel_player_truck(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
) -> Result<(), String> {
    dev_log!("Refueling player truck");
    generic_vehicle_attribute_edit(
        profile_state,
        decrypt_cache,
        profile_cache,
        "vehicle",
        "my_truck",
        "fuel_relative",
        |_| float_to_hex(1.0),
    )
}

#[command]
pub async fn set_player_truck_fuel(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
    level: f32,
) -> Result<(), String> {
    dev_log!("Set Fuel player truck");
    generic_vehicle_attribute_edit(
        profile_state,
        decrypt_cache,
        profile_cache,
        "vehicle",
        "my_truck",
        "fuel_relative",
        |_| float_to_hex(level),
    )
}

#[command]
pub async fn set_player_truck_wear(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
    wear_type: String,
    level: f32,
) -> Result<(), String> {
    dev_log!("Set wear for player truck: {} = {}", wear_type, level);
    generic_vehicle_attribute_edit(
        profile_state,
        decrypt_cache,
        profile_cache,
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
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
) -> Result<(), String> {
    dev_log!("Setting trailer license plate to: {}", plate);
    generic_vehicle_attribute_edit(
        profile_state,
        decrypt_cache,
        profile_cache,
        "trailer",
        "my_trailer",
        "license_plate",
        |caps: &Captures| {
            let old_value = &caps[2];
            let old_value_unquoted = old_value.trim_matches('"');
            if let Some(pipe_index) = old_value_unquoted.rfind('|') {
                let country_part = &old_value_unquoted[pipe_index + 1..];
                format!(r#""{}|{}""#, &plate, country_part)
            } else {
                format!(r#""{}""#, &plate)
            }
        },
    )
}

#[command]
pub async fn edit_truck_odometer(
    value: i64,
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
) -> Result<(), String> {
    dev_log!("Setting truck odometer to: {}", value);
    generic_vehicle_attribute_edit(
        profile_state,
        decrypt_cache,
        profile_cache,
        "vehicle",
        "my_truck",
        "odometer",
        |_| value.to_string(),
    )
}

#[command]
pub async fn repair_player_trailer(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
) -> Result<(), String> {
    dev_log!("Repairing player trailer");
    let (content, path) = read_save_content(profile_state, decrypt_cache.clone())?;
    let trailer_id = get_player_vehicle_id(&content, "my_trailer")?;

    dev_log!("Found trailer ID: {}", trailer_id);

    // ← CHANGED: Use proper brace matching to get complete block
    let (block_start, block_end) = extract_vehicle_block(&content, "trailer", &trailer_id)?;
    let mut block = content[block_start..block_end].to_string();

    dev_log!("Extracted trailer block length: {}", block.len());

    // Note: In SII files, trailer body wear is called "trailer_body_wear", not just "body_wear"
    let wear_attributes = [
        "chassis_wear",
        "trailer_body_wear",  // ← IMPORTANT: Correct attribute name!
    ];

    for attr in &wear_attributes {
        let regex_str = format!(r"({}:\s*)([^ \r\n]+)", attr);
        let re = Regex::new(&regex_str).map_err(|e| e.to_string())?;
        
        if re.is_match(&block) {
            dev_log!("Repairing {} to 0.0", attr);
            block = re.replace(&block, format!("$1{}", float_to_hex(0.0))).to_string();
        } else {
            dev_log!("Warning: {} not found in trailer block", attr);
        }
    }

    // Fix wheels_wear array - match each individual wheel
    let re_wheels = Regex::new(r"(wheels_wear\[\d+\]:\s*)([^ \r\n]+)").unwrap();
    if re_wheels.is_match(&block) {
        dev_log!("Repairing trailer wheels");
        block = re_wheels.replace_all(&block, |caps: &Captures| {
            format!("{}{}", &caps[1], float_to_hex(0.0))
        }).to_string();
    } else {
        dev_log!("Warning: wheels_wear not found in trailer block");
    }

    let new_content = format!("{}{}{}", &content[..block_start], block, &content[block_end..]);
    
    dev_log!("Writing repaired trailer back to file");
    write_save_content(&path, &new_content)?;

    decrypt_cache.invalidate_path(Path::new(&path));
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();

    Ok(())
}

#[command]
pub async fn set_player_trailer_cargo_mass(
    mass: f32,
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
) -> Result<(), String> {
    dev_log!("Setting trailer cargo mass to: {}", mass);
    generic_vehicle_attribute_edit(
        profile_state,
        decrypt_cache,
        profile_cache,
        "trailer",
        "my_trailer",
        "cargo_mass",
        |_| float_to_hex(mass),
    )
}
