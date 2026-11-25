use crate::dev_log;
use crate::features::backup::service as backup_service;
use crate::features::logging::service as logging_service;
use crate::features::trailer_change::parser::{
    ParsedTrailerSave, find_trailer_block_by_id, find_unit_block_by_id, parse_trailer_save,
    resolve_current_trailer_pointer,
};
use crate::features::truck_change::parser::{extract_field_value, is_null_ref};
use crate::shared::decrypt::decrypt_cached;
use crate::shared::hex_float::float_to_hex;
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
    let path = crate::features::save_editor::safety::active_save(profile_state.inner())?;
    decrypt_cache.invalidate_path(&path);
    let content = decrypt_cached(&path, &decrypt_cache)?;
    Ok((content, path.display().to_string()))
}

fn write_save_content(
    profile_state: &AppProfileState,
    path: &str,
    content: &str,
    original: &str,
    action: &str,
    action_reason: &str,
    success_message: &str,
) -> Result<(), String> {
    let path_buf = PathBuf::from(path);
    let _mutation = crate::features::save_editor::safety::mutation_lock()?;
    let raw = crate::features::save_editor::safety::read_source(profile_state, &path_buf)?;
    if crate::shared::decrypt::decode_text_bytes(&raw, "vehicle_save", &[])? != original {
        return Err("save_changed_since_load".into());
    }
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
    crate::features::save_editor::safety::write_atomic_checked(&path_buf, &raw, content, |_| {
        Ok(())
    })?;
    crate::features::save_editor::safety::remember_save(profile_state, &path_buf)?;

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

/// Liest die ID des `economy.player`-Verweises. Eigenständige, minimale Implementierung
/// (statt eines Imports aus einem anderen Parser-Modul), damit dieses Modul unabhängig bleibt.
fn get_player_id(content: &str) -> Option<String> {
    let mut in_economy = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("economy :") {
            in_economy = true;
        }
        if in_economy && trimmed.starts_with("player:") {
            return trimmed.split_whitespace().nth(1).map(|s| s.to_string());
        }
        if in_economy && trimmed.starts_with('}') {
            in_economy = false;
        }
    }
    None
}

/// Findet die ID des Spieler-Fahrzeugs (Truck oder Trailer).
///
/// WICHTIG: `player.my_truck` / `player.my_trailer` sind in aktuellen ETS2-Saves
/// praktisch immer `null` (siehe Logs: `player_my_truck: null, player_my_trailer: null`).
/// Die verlässliche Quelle ist stattdessen:
///   player.assigned_vehicles → player_vehicles-Block → `vehicle:`-/`trailer:`-Feld
///
/// `player_field` bestimmt NUR noch, ob Truck ("vehicle") oder Trailer ("trailer")
/// gewünscht ist – über die Werte "my_truck" / "my_trailer", wie an den Call-Sites üblich.
fn get_player_vehicle_id(content: &str, player_field: &str) -> Result<String, String> {
    let target_key = match player_field {
        "my_truck" => "vehicle",
        "my_trailer" => "trailer",
        other => other, // Fallback: falls direkt "vehicle"/"trailer" übergeben wird
    };

    let player_id = get_player_id(content).ok_or_else(|| "player_not_found".to_string())?;

    let (player_start, player_end) = extract_vehicle_block(content, "player", &player_id)
        .map_err(|_| "player_block_not_found".to_string())?;
    let player_block = &content[player_start..player_end];

    if extract_field_value(player_block, "assigned_vehicles_mode")
        .is_some_and(|mode| mode != "truck")
    {
        return Err("vehicle_mode_not_supported".into());
    }
    let assigned_vehicles_id =
        extract_field_value(player_block, "assigned_vehicles").filter(|id| !is_null_ref(id));
    let Some(assigned_vehicles_id) = assigned_vehicles_id else {
        let id = extract_field_value(player_block, player_field)
            .filter(|id| !is_null_ref(id))
            .ok_or("assigned_vehicles_not_found")?;
        let kind = if target_key == "vehicle" {
            "vehicle"
        } else {
            "trailer"
        };
        extract_vehicle_block(content, kind, &id)?;
        return Ok(id);
    };

    let (pv_start, pv_end) =
        extract_vehicle_block(content, "player_vehicles", &assigned_vehicles_id)
            .map_err(|_| "player_vehicles_block_not_found".to_string())?;
    let pv_block = &content[pv_start..pv_end];

    extract_field_value(pv_block, target_key)
        .filter(|value| !is_null_ref(value))
        .ok_or_else(|| format!("{} nicht gefunden", player_field))
}

// Extract complete vehicle/trailer/player/player_vehicles block with proper brace matching.
// Generic über block_type, funktioniert daher auch für "player" und "player_vehicles".
fn extract_vehicle_block(
    content: &str,
    block_type: &str,
    vehicle_id: &str,
) -> Result<(usize, usize), String> {
    let units = crate::features::save_editor::document::scan_units(content, false)?;
    let mut units = units
        .iter()
        .filter(|unit| unit.kind == block_type && unit.id == vehicle_id);
    let unit = units
        .next()
        .ok_or_else(|| format!("vehicle_reference_missing:{block_type}"))?;
    if units.next().is_some() {
        return Err("vehicle_reference_ambiguous".into());
    }
    Ok((unit.span.start, unit.span.end))
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

    let document = crate::shared::sii_document::Document::parse(&content)?;
    let unit = document.resolve(&vehicle_id, unit_type)?;
    let old = document.field(unit, attribute_key)?;
    let expression = format!("{attribute_key}: {old}");
    let regex = Regex::new(r"^([^:]+:[ \t]*)(.*)$").map_err(|error| error.to_string())?;
    let captures = regex.captures(&expression).ok_or("vehicle_field_invalid")?;
    let new_content = document.patch(unit, attribute_key, &value_setter(&captures))?;
    write_save_content(
        profile_state.inner(),
        &path,
        &new_content,
        &content,
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
    let document = crate::shared::sii_document::Document::parse(&content)?;
    let unit = document.resolve(&trailer_id, "trailer")?;
    let old = document.field(unit, attribute_key)?;
    let expression = format!("{attribute_key}: {old}");
    let regex = Regex::new(r"^([^:]+:[ \t]*)(.*)$").map_err(|error| error.to_string())?;
    let captures = regex.captures(&expression).ok_or("vehicle_field_invalid")?;
    let new_content = document.patch(unit, attribute_key, &value_setter(&captures))?;
    write_save_content(
        profile_state.inner(),
        &path,
        &new_content,
        &content,
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

fn repair_unit(content: &str, kind: &str, id: &str, fields: &[&str]) -> Result<String, String> {
    let document = crate::shared::sii_document::Document::parse(content)?;
    let unit = document.resolve(id, kind)?;
    let mut spans = Vec::new();
    for field in fields {
        match document.field_span(unit, field) {
            Ok(span) => {
                crate::shared::hex_float::parse_value_auto(&content[span.clone()])?;
                spans.push(span);
            }
            Err(error) if error == format!("sii_field_missing:{field}") => {}
            Err(error) => return Err(error),
        }
    }
    match document.field(unit, "wheels_wear") {
        Ok(count) => {
            let count = count
                .parse::<usize>()
                .map_err(|_| "vehicle_wheel_array_invalid")?;
            if count > 128 {
                return Err("vehicle_wheel_array_invalid".into());
            }
            let raw = &content[unit.span.clone()];
            let entries =
                crate::features::truck_change::parser::extract_array_entries(raw, "wheels_wear");
            if entries.len() != count
                || entries
                    .iter()
                    .enumerate()
                    .any(|(index, (actual, _))| index != *actual)
            {
                return Err("vehicle_wheel_array_invalid".into());
            }
            for index in 0..count {
                spans.push(document.field_span(unit, &format!("wheels_wear[{index}]"))?);
            }
        }
        Err(error) if error == "sii_field_missing:wheels_wear" => {}
        Err(error) => return Err(error),
    }
    if spans.is_empty() {
        return Err("vehicle_repair_fields_missing".into());
    }
    spans.sort_by_key(|span| std::cmp::Reverse(span.start));
    let mut updated = content.to_string();
    for span in spans {
        updated.replace_range(span, &float_to_hex(0.0));
    }
    crate::shared::sii_document::Document::parse(&updated)?;
    Ok(updated)
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
    let plate = validate_trailer_license_plate(&plate)?;
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

    let new_content = repair_unit(
        &content,
        "vehicle",
        &truck_id,
        &[
            "engine_wear",
            "transmission_wear",
            "cabin_wear",
            "chassis_wear",
        ],
    )?;
    write_save_content(
        profile_state.inner(),
        &path,
        &new_content,
        &content,
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
    if !level.is_finite() || !(0.0..=1.0).contains(&level) {
        return Err("vehicle_value_invalid".into());
    }
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
    if !level.is_finite()
        || !(0.0..=1.0).contains(&level)
        || ![
            "engine_wear",
            "transmission_wear",
            "cabin_wear",
            "chassis_wear",
        ]
        .contains(&wear_type.as_str())
    {
        return Err("vehicle_value_invalid".into());
    }
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
    if !(0..=9_007_199_254_740_991).contains(&value) {
        return Err("vehicle_value_invalid".into());
    }
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

    let new_content = repair_unit(
        &content,
        "trailer",
        &trailer_id,
        &["chassis_wear", "trailer_body_wear"],
    )?;
    write_save_content(
        profile_state.inner(),
        &path,
        &new_content,
        &content,
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

    #[test]
    fn ats_repair_keeps_wheel_indices_unknown_fields_and_other_units() {
        let original = crate::features::save_editor::ats_tests::fixture().replace('\n', "\r\n");
        let repaired = repair_unit(
            &original,
            "vehicle",
            "truck.main",
            &[
                "engine_wear",
                "transmission_wear",
                "cabin_wear",
                "chassis_wear",
            ],
        )
        .unwrap();
        let doc = crate::shared::sii_document::Document::parse(&repaired).unwrap();
        let truck = doc.resolve("truck.main", "vehicle").unwrap();
        assert_eq!(doc.field(truck, "wheels_wear").unwrap(), "2");
        assert_eq!(doc.field(truck, "wheels_wear[0]").unwrap(), "&00000000");
        assert_eq!(doc.field(truck, "wheels_wear[1]").unwrap(), "&00000000");
        assert_eq!(doc.field(truck, "fuel_relative").unwrap(), "&3f000000");
        assert_eq!(doc.field(truck, "odometer").unwrap(), "574021");
        assert!(repaired.contains("bus_experience_points: 12\r\n"));
        let broken = original.replace("wheels_wear[1]", "wheels_wear[0]");
        assert!(repair_unit(&broken, "vehicle", "truck.main", &["engine_wear"]).is_err());
    }

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

    fn truck_fixture() -> &'static str {
        r#"SiiNunit
{
economy : _nameless.economy {
 player: _nameless.player
}
player : _nameless.player {
 assigned_vehicles: _nameless.assigned.1
 my_truck: null
 my_trailer: null
}
player_vehicles : _nameless.assigned.1 {
 vehicle: _nameless.truck.active
 trailer: _nameless.trailer.active
}
vehicle : _nameless.truck.active {
 fuel_relative: &3edf8ac0
 engine_wear: &3be51ba8
 odometer: 574021
}
trailer : _nameless.trailer.active {
 license_plate: "ACTIVE|germany"
}
}
"#
    }

    #[test]
    fn resolves_player_truck_id_via_assigned_vehicles() {
        assert_eq!(
            get_player_vehicle_id(truck_fixture(), "my_truck").unwrap(),
            "_nameless.truck.active"
        );
    }

    #[test]
    fn resolves_player_trailer_id_via_assigned_vehicles() {
        assert_eq!(
            get_player_vehicle_id(truck_fixture(), "my_trailer").unwrap(),
            "_nameless.trailer.active"
        );
    }

    #[test]
    fn rejects_truck_id_when_assigned_vehicles_is_null() {
        let fixture = truck_fixture().replace(
            "assigned_vehicles: _nameless.assigned.1",
            "assigned_vehicles: null",
        );
        assert_eq!(
            get_player_vehicle_id(&fixture, "my_truck").unwrap_err(),
            "assigned_vehicles_not_found"
        );
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
