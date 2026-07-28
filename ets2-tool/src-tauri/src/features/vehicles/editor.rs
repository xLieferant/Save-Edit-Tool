use crate::dev_log;
use crate::features::backup::service as backup_service;
use crate::features::logging::service as logging_service;
use crate::features::trailer_change::parser::{
    ParsedTrailerSave, find_trailer_block_by_id, find_unit_block_by_id, parse_trailer_save,
    resolve_current_trailer_pointer,
};
use crate::features::truck_change::parser::{extract_field_value, is_null_ref};
use crate::shared::current_profile::{require_current_profile, require_current_save};
use crate::shared::decrypt::decrypt_cached;
use crate::shared::hex_float::float_to_hex;
use crate::shared::paths::game_sii_from_save;
use crate::shared::regex_helper::cragex;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use regex::{Captures, Regex};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::command;

const MAX_TRAILER_LICENSE_PLATE_CHARS: usize = 32;
const MAX_JOB_WEIGHT_KG: f32 = 1_000_000.0;

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
    Ok((content, path.display().to_string()))
}

fn write_save_content(
    profile_state: &AppProfileState,
    path: &str,
    content: &str,
    action: &str,
    action_reason: &str,
    success_message: &str,
) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    let mut context = logging_service::resolve_active_context(profile_state);
    context.extra.insert(
        "target".to_string(),
        logging_service::redact_path(&path_buf.display().to_string()),
    );
    context
        .extra
        .insert("reason".to_string(), action_reason.to_string());

    let backup = backup_service::create_backup_for_targets(
        profile_state,
        action_reason,
        &backup_service::recommended_targets(&path_buf),
    )
    .map_err(|error| {
        let _ = logging_service::record_error(
            action,
            Some("auto_backup_failed"),
            "Automatic backup could not be created before the vehicle edit.",
            Some(&error),
            &context,
        );
        "Automatisches Backup konnte vor der Fahrzeugänderung nicht erstellt werden.".to_string()
    })?;

    context
        .extra
        .insert("backupId".to_string(), backup.backup_id);
    fs::write(&path_buf, content.as_bytes()).map_err(|error| {
        let technical = error.to_string();
        let _ = logging_service::record_error(
            action,
            Some("write_failed"),
            "The vehicle save could not be written.",
            Some(&technical),
            &context,
        );
        "Fahrzeugänderung konnte nicht gespeichert werden.".to_string()
    })?;

    let _ = logging_service::record_info(action, success_message, &context);
    Ok(())
}

fn verify_written_content(
    profile_state: &AppProfileState,
    path: &str,
    expected: &str,
    action: &str,
) -> Result<(), String> {
    let actual = fs::read_to_string(path).map_err(|error| {
        let context = logging_service::resolve_active_context(profile_state);
        let technical = error.to_string();
        let _ = logging_service::record_error(
            action,
            Some("write_verification_failed"),
            "The trailer save could not be read back after writing.",
            Some(&technical),
            &context,
        );
        "trailer_write_verification_failed".to_string()
    })?;

    if actual != expected {
        let context = logging_service::resolve_active_context(profile_state);
        let _ = logging_service::record_error(
            action,
            Some("write_verification_mismatch"),
            "The trailer save did not match the requested content after writing.",
            None,
            &context,
        );
        return Err("trailer_write_verification_failed".to_string());
    }

    Ok(())
}

fn editable_active_trailer_id(parsed: &ParsedTrailerSave) -> Result<String, String> {
    let pointer = resolve_current_trailer_pointer(parsed)?;
    if !pointer.writable {
        return Err("active_trailer_not_editable".to_string());
    }
    let trailer_block = find_trailer_block_by_id(&parsed.trailer_blocks, &pointer.trailer_id)
        .ok_or_else(|| "active_trailer_not_found".to_string())?;
    dev_log!(
        "Resolved active trailer from {} with {} confidence",
        pointer.source,
        pointer.confidence
    );
    Ok(trailer_block.id.clone())
}

fn resolve_editable_active_trailer_id(content: &str) -> Result<String, String> {
    let parsed = parse_trailer_save(content);
    editable_active_trailer_id(&parsed)
}

fn resolve_active_job_trailer_id(content: &str) -> Result<String, String> {
    let parsed = parse_trailer_save(content);
    let player_id = parsed
        .player_id
        .as_deref()
        .ok_or_else(|| "player_not_found".to_string())?;
    let player_block = find_unit_block_by_id(&parsed.unit_blocks, player_id, Some("player"))
        .ok_or_else(|| "player_not_found".to_string())?;
    let job_id = extract_field_value(&player_block.raw_block, "current_job")
        .filter(|value| !is_null_ref(value))
        .ok_or_else(|| "no_active_job".to_string())?;
    let job_block = find_unit_block_by_id(&parsed.unit_blocks, &job_id, None)
        .ok_or_else(|| "no_active_job".to_string())?;

    if let Some(company_trailer_id) = extract_field_value(&job_block.raw_block, "company_trailer")
        .filter(|value| !is_null_ref(value))
    {
        let trailer_block = find_trailer_block_by_id(&parsed.trailer_blocks, &company_trailer_id)
            .ok_or_else(|| "active_job_trailer_not_found".to_string())?;
        dev_log!("Resolved active job company trailer");
        return Ok(trailer_block.id.clone());
    }

    editable_active_trailer_id(&parsed)
}

fn validate_trailer_license_plate(plate: &str) -> Result<String, String> {
    let trimmed = plate.trim();
    if trimmed.is_empty() {
        return Err("trailer_license_plate_empty".to_string());
    }
    if trimmed.chars().count() > MAX_TRAILER_LICENSE_PLATE_CHARS {
        return Err("trailer_license_plate_too_long".to_string());
    }
    if trimmed
        .chars()
        .any(|character| character.is_control() || matches!(character, '"' | '\\' | '|'))
    {
        return Err("trailer_license_plate_invalid".to_string());
    }

    Ok(trimmed.to_string())
}

fn validate_job_weight(mass: f32) -> Result<f32, String> {
    if !mass.is_finite() || !(0.0..=MAX_JOB_WEIGHT_KG).contains(&mass) {
        return Err("job_weight_invalid".to_string());
    }

    Ok(mass)
}

fn get_player_vehicle_id(content: &str, vehicle_type: &str) -> Result<String, String> {
    let regex_str = format!(
        r"player\s*:\s*[A-Za-z0-9._]+\s*\{{\s*[^}}]*?{}\s*:\s*([A-Za-z0-9._]+)",
        vehicle_type
    );
    let re = cragex(&regex_str).map_err(|e| format!("Regex Fehler: {}", e))?;
    re.captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| format!("{} nicht gefunden", vehicle_type))
}

// ← NEW: Extract complete vehicle/trailer block with proper brace matching
fn extract_vehicle_block(
    content: &str,
    block_type: &str,
    vehicle_id: &str,
) -> Result<(usize, usize), String> {
    let start_pattern = format!(r"{}\s*:\s*{}\s*\{{", block_type, regex::escape(vehicle_id));
    let re_start = Regex::new(&start_pattern).map_err(|e| e.to_string())?;

    let cap = re_start
        .captures(content)
        .ok_or(format!("{} block for {} not found", block_type, vehicle_id))?;

    let full_match = cap.get(0).ok_or_else(|| {
        format!(
            "{} block start for {} could not be resolved",
            block_type, vehicle_id
        )
    })?;
    let start_pos = full_match.end();

    // Count braces to find the matching closing brace
    let mut brace_count = 1;
    let mut end_pos = start_pos;
    for (byte_offset, ch) in content[start_pos..].char_indices() {
        if ch == '{' {
            brace_count += 1;
        } else if ch == '}' {
            brace_count -= 1;
            if brace_count == 0 {
                end_pos = start_pos + byte_offset;
                break;
            }
        }
    }

    if brace_count != 0 {
        return Err(format!("Unmatched braces in {} block", block_type));
    }

    // Return positions INCLUDING the opening brace position
    Ok((full_match.start(), end_pos + 1))
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
    action: &str,
    action_reason: &str,
    success_message: &str,
    unit_type: &str,          // "vehicle" or "trailer"
    player_vehicle_key: &str, // "my_truck" or "my_trailer"
    attribute_key: &str,
    value_setter: F,
) -> Result<(), String>
where
    F: Fn(&Captures) -> String,
{
    let (content, path) = read_save_content(profile_state.clone(), decrypt_cache.clone())?;
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

    let new_content = format!(
        "{}{}{}",
        &content[..block_start],
        new_block,
        &content[block_end..]
    );
    write_save_content(
        profile_state.inner(),
        &path,
        &new_content,
        action,
        action_reason,
        success_message,
    )?;

    decrypt_cache.invalidate_path(Path::new(&path));
    profile_cache.invalidate_save_data();
    profile_cache.invalidate_vehicle_data();

    Ok(())
}

fn edit_resolved_trailer_attribute<F>(
    profile_state: tauri::State<'_, AppProfileState>,
    decrypt_cache: tauri::State<'_, DecryptCache>,
    profile_cache: tauri::State<'_, ProfileCache>,
    action: &str,
    action_reason: &str,
    success_message: &str,
    attribute_key: &str,
    resolve_trailer_id: fn(&str) -> Result<String, String>,
    value_setter: F,
) -> Result<(), String>
where
    F: Fn(&Captures) -> String,
{
    let (content, path) = read_save_content(profile_state.clone(), decrypt_cache.clone())?;
    let trailer_id = resolve_trailer_id(&content)?;
    let (block_start, block_end) = extract_vehicle_block(&content, "trailer", &trailer_id)?;
    let block = &content[block_start..block_end];
    let regex_str = format!(r"({}:\s*)([^\r\n]+)", attribute_key);
    let re = Regex::new(&regex_str).map_err(|error| error.to_string())?;

    if !re.is_match(block) {
        return Err(format!("trailer_attribute_not_found:{}", attribute_key));
    }

    let new_block = re.replace(block, |captures: &Captures| {
        format!("{}{}", &captures[1], value_setter(captures))
    });
    let new_content = format!(
        "{}{}{}",
        &content[..block_start],
        new_block,
        &content[block_end..]
    );
    write_save_content(
        profile_state.inner(),
        &path,
        &new_content,
        action,
        action_reason,
        success_message,
    )?;
    verify_written_content(profile_state.inner(), &path, &new_content, action)?;

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
        "set_player_truck_license_plate",
        "before truck license plate edit",
        "The player truck license plate was updated.",
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
    let (content, path) = read_save_content(profile_state.clone(), decrypt_cache.clone())?;
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
            block = re
                .replace(&block, format!("$1{}", float_to_hex(0.0)))
                .to_string();
        }
    }

    // Fix wheels_wear array
    let re_wheels =
        Regex::new(r"wheels_wear\[\d+\]:\s*[^ \r\n]+").map_err(|error| error.to_string())?;
    block = re_wheels
        .replace_all(&block, |_: &Captures| {
            format!("wheels_wear[0]: {}", float_to_hex(0.0))
        })
        .to_string();

    let new_content = format!(
        "{}{}{}",
        &content[..block_start],
        block,
        &content[block_end..]
    );
    write_save_content(
        profile_state.inner(),
        &path,
        &new_content,
        "repair_player_truck",
        "before truck repair",
        "The player truck wear values were repaired.",
    )?;

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
        "refuel_player_truck",
        "before truck refuel edit",
        "The player truck fuel level was restored.",
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
        "set_player_truck_fuel",
        "before truck fuel edit",
        "The player truck fuel level was updated.",
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
        "set_player_truck_wear",
        "before truck wear edit",
        "A player truck wear value was updated.",
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
    let plate = validate_trailer_license_plate(&plate)?;
    dev_log!(
        "Setting trailer license plate ({} characters)",
        plate.chars().count()
    );
    edit_resolved_trailer_attribute(
        profile_state,
        decrypt_cache,
        profile_cache,
        "set_player_trailer_license_plate",
        "before trailer license plate edit",
        "The player trailer license plate was updated.",
        "license_plate",
        resolve_editable_active_trailer_id,
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
        "edit_truck_odometer",
        "before truck odometer edit",
        "The player truck odometer was updated.",
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
    let (content, path) = read_save_content(profile_state.clone(), decrypt_cache.clone())?;
    let trailer_id = resolve_editable_active_trailer_id(&content)?;

    dev_log!("Found trailer ID: {}", trailer_id);

    // ← CHANGED: Use proper brace matching to get complete block
    let (block_start, block_end) = extract_vehicle_block(&content, "trailer", &trailer_id)?;
    let mut block = content[block_start..block_end].to_string();

    dev_log!("Extracted trailer block length: {}", block.len());

    // Note: In SII files, trailer body wear is called "trailer_body_wear", not just "body_wear"
    let wear_attributes = [
        "chassis_wear",
        "trailer_body_wear", // ← IMPORTANT: Correct attribute name!
    ];
    let mut repaired_any = false;

    for attr in &wear_attributes {
        let regex_str = format!(r"({}:\s*)([^ \r\n]+)", attr);
        let re = Regex::new(&regex_str).map_err(|e| e.to_string())?;

        if re.is_match(&block) {
            repaired_any = true;
            dev_log!("Repairing {} to 0.0", attr);
            block = re
                .replace(&block, format!("$1{}", float_to_hex(0.0)))
                .to_string();
        } else {
            dev_log!("Warning: {} not found in trailer block", attr);
        }
    }

    // Fix wheels_wear array - match each individual wheel
    let re_wheels =
        Regex::new(r"(wheels_wear\[\d+\]:\s*)([^ \r\n]+)").map_err(|error| error.to_string())?;
    if re_wheels.is_match(&block) {
        repaired_any = true;
        dev_log!("Repairing trailer wheels");
        block = re_wheels
            .replace_all(&block, |caps: &Captures| {
                format!("{}{}", &caps[1], float_to_hex(0.0))
            })
            .to_string();
    } else {
        dev_log!("Warning: wheels_wear not found in trailer block");
    }

    if !repaired_any {
        return Err("trailer_repair_fields_not_found".to_string());
    }

    let new_content = format!(
        "{}{}{}",
        &content[..block_start],
        block,
        &content[block_end..]
    );

    dev_log!("Writing repaired trailer back to file");
    write_save_content(
        profile_state.inner(),
        &path,
        &new_content,
        "repair_player_trailer",
        "before trailer repair",
        "The player trailer wear values were repaired.",
    )?;
    verify_written_content(
        profile_state.inner(),
        &path,
        &new_content,
        "repair_player_trailer",
    )?;

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
    let mass = validate_job_weight(mass)?;
    dev_log!("Setting trailer cargo mass to: {}", mass);
    edit_resolved_trailer_attribute(
        profile_state,
        decrypt_cache,
        profile_cache,
        "set_player_trailer_cargo_mass",
        "before trailer cargo mass edit",
        "The player trailer cargo mass was updated.",
        "cargo_mass",
        resolve_active_job_trailer_id,
        |_| float_to_hex(mass),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trailer_fixture() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 assigned_vehicles: _nameless.assigned.1
 assigned_trailer: _nameless.trailer.active
 my_trailer: null
 current_job: _nameless.job
 trailers: 2
 trailers[0]: _nameless.trailer.active
 trailers[1]: _nameless.trailer.company
}
player_vehicles : _nameless.assigned.1 {
 vehicle: _nameless.truck.active
 trailer: _nameless.trailer.active
}
player_job : _nameless.job {
 company_trailer: _nameless.trailer.company
}
trailer : _nameless.trailer.active {
 license_plate: "ACTIVE|germany"
 cargo_mass: &00000000
 chassis_wear: &3f000000
}
trailer : _nameless.trailer.company {
 license_plate: "JOB|germany"
 cargo_mass: &00000000
 chassis_wear: &3f000000
}
}
"#
    }

    #[test]
    fn resolves_active_trailer_from_assigned_vehicles() {
        assert_eq!(
            resolve_editable_active_trailer_id(trailer_fixture()).unwrap(),
            "_nameless.trailer.active"
        );
    }

    #[test]
    fn resolves_company_trailer_for_active_job() {
        assert_eq!(
            resolve_active_job_trailer_id(trailer_fixture()).unwrap(),
            "_nameless.trailer.company"
        );
    }

    #[test]
    fn rejects_job_weight_without_active_job() {
        let fixture = trailer_fixture().replace("current_job: _nameless.job", "current_job: null");
        assert_eq!(
            resolve_active_job_trailer_id(&fixture).unwrap_err(),
            "no_active_job"
        );
    }

    #[test]
    fn validates_trailer_license_plate() {
        assert_eq!(
            validate_trailer_license_plate("  MÜN-Ä 123  ").unwrap(),
            "MÜN-Ä 123"
        );
        assert_eq!(
            validate_trailer_license_plate(" ").unwrap_err(),
            "trailer_license_plate_empty"
        );
        assert_eq!(
            validate_trailer_license_plate("BAD|PLATE").unwrap_err(),
            "trailer_license_plate_invalid"
        );
        assert_eq!(
            validate_trailer_license_plate(&"X".repeat(33)).unwrap_err(),
            "trailer_license_plate_too_long"
        );
    }

    #[test]
    fn extracts_trailer_block_with_unicode_plate() {
        let content = r#"trailer : _nameless.trailer.active {
 license_plate: "MÜN-Ä 123|germany"
 cargo_mass: &00000000
}
economy : _nameless.economy {
 player: _nameless.player
}"#;
        let (start, end) =
            extract_vehicle_block(content, "trailer", "_nameless.trailer.active").unwrap();

        assert_eq!(
            &content[start..end],
            r#"trailer : _nameless.trailer.active {
 license_plate: "MÜN-Ä 123|germany"
 cargo_mass: &00000000
}"#
        );
    }

    #[test]
    fn validates_job_weight_range_and_decimals() {
        assert_eq!(validate_job_weight(0.0).unwrap(), 0.0);
        assert_eq!(validate_job_weight(12_345.5).unwrap(), 12_345.5);
        assert_eq!(
            validate_job_weight(MAX_JOB_WEIGHT_KG).unwrap(),
            MAX_JOB_WEIGHT_KG
        );
        assert_eq!(validate_job_weight(-0.1).unwrap_err(), "job_weight_invalid");
        assert_eq!(
            validate_job_weight(MAX_JOB_WEIGHT_KG + 1.0).unwrap_err(),
            "job_weight_invalid"
        );
        assert_eq!(
            validate_job_weight(f32::NAN).unwrap_err(),
            "job_weight_invalid"
        );
        assert_eq!(
            validate_job_weight(f32::INFINITY).unwrap_err(),
            "job_weight_invalid"
        );
    }
}
