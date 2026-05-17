use crate::dev_log;
use crate::models::quicksave_game_info::{CurrentTruckSummary, GameDataQuicksave};
use crate::shared::decrypt::decrypt_cached_with_cache;
use crate::shared::paths::game_sii_from_save;
use crate::shared::regex_helper::cragex;
use crate::shared::trace::TraceScope;
use crate::state::{AppProfileState, DecryptCache, ProfileCache};
use std::path::{Path, PathBuf};
use tauri::command;
use tauri::State;

#[command]
pub async fn quicksave_game_info(
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<GameDataQuicksave, String> {
    let mut trace = TraceScope::new("quicksave_game_info");
    let current_profile = profile_state
        .current_profile
        .lock()
        .map_err(|_| "AppProfileState current_profile lock poisoned".to_string())?
        .clone();
    let current_save = profile_state
        .current_save
        .lock()
        .map_err(|_| "AppProfileState current_save lock poisoned".to_string())?
        .clone();
    let save = resolve_active_save_from_snapshot(current_save, current_profile)?;
    let path = game_sii_from_save(Path::new(&save));
    let path_key = path.display().to_string();

    if let Some(cached) = profile_cache.get_quicksave_data(&path_key) {
        dev_log!("quicksave_game_info cache hit");
        trace.finish_ok();
        return Ok(cached);
    }

    let decrypt_cache = decrypt_cache.inner().clone();
    let path_for_worker = path.clone();
    let (result, truck_summary) = tauri::async_runtime::spawn_blocking(move || {
        build_quicksave_game_info_from_path(path_for_worker, &decrypt_cache)
    })
    .await
    .map_err(|error| format!("quicksave_game_info join failed: {}", error))??;

    profile_cache.cache_current_truck_summary(path_key.clone(), truck_summary);
    dev_log!("quicksave_game_info parsed");
    profile_cache.cache_quicksave_data(path_key, result.clone());
    trace.finish_ok();
    Ok(result)
}

#[command]
pub async fn get_current_truck_summary(
    profile_state: State<'_, AppProfileState>,
    profile_cache: State<'_, ProfileCache>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<Option<CurrentTruckSummary>, String> {
    let mut trace = TraceScope::new("get_current_truck_summary");
    let current_profile = profile_state
        .current_profile
        .lock()
        .map_err(|_| "AppProfileState current_profile lock poisoned".to_string())?
        .clone();
    let current_save = profile_state
        .current_save
        .lock()
        .map_err(|_| "AppProfileState current_save lock poisoned".to_string())?
        .clone();
    let save = resolve_active_save_from_snapshot(current_save, current_profile)?;
    let path = game_sii_from_save(Path::new(&save));
    let path_key = path.display().to_string();

    dev_log!("truck_summary active save: {}", save);

    if let Some(cached) = profile_cache.get_current_truck_summary(&path_key) {
        dev_log!("truck_summary cache hit");
        trace.finish_ok();
        return Ok(cached);
    }

    if let Some(quicksave) = profile_cache.get_quicksave_data(&path_key) {
        let summary = summary_from_quicksave(&quicksave);
        profile_cache.cache_current_truck_summary(path_key, summary.clone());
        dev_log!("truck_summary cache hit");
        trace.finish_ok();
        return Ok(summary);
    }

    dev_log!("truck_summary cache miss");

    if !path.is_file() {
        profile_cache.cache_current_truck_summary(path_key, None);
        trace.finish_ok();
        return Ok(None);
    }

    let decrypt_cache = decrypt_cache.inner().clone();
    let path_for_worker = path.clone();
    let summary = tauri::async_runtime::spawn_blocking(move || {
        build_current_truck_summary_from_path(path_for_worker, &decrypt_cache)
    })
    .await
    .map_err(|error| format!("get_current_truck_summary join failed: {}", error))??;

    dev_log!("truck_summary parsed successfully");
    profile_cache.cache_current_truck_summary(path_key, summary.clone());
    trace.finish_ok();
    Ok(summary)
}

fn resolve_active_save_from_snapshot(
    current_save: Option<String>,
    current_profile: Option<String>,
) -> Result<String, String> {
    if let Some(save) = current_save {
        return Ok(save);
    }
    let profile = current_profile.ok_or_else(|| "Kein Profil geladen.".to_string())?;
    Ok(format!("{}/save/quicksave", profile))
}

fn summary_from_quicksave(data: &GameDataQuicksave) -> Option<CurrentTruckSummary> {
    let summary = CurrentTruckSummary {
        brand_label: data.truck_brand_label.clone(),
        model_label: data.truck_model_label.clone(),
        display_name: data.truck_display_name.clone(),
        odometer_km: data.odometer,
        cleaned_plate: data.license_plate.clone(),
    };

    if summary.brand_label.is_none()
        && summary.model_label.is_none()
        && summary.display_name.is_none()
        && summary.odometer_km.is_none()
        && summary.cleaned_plate.is_none()
    {
        None
    } else {
        Some(summary)
    }
}

fn parse_current_truck_summary_from_content(
    content: &str,
) -> Result<Option<CurrentTruckSummary>, String> {
    let (_, player_block) = match extract_first_block(content, "player")? {
        Some(value) => value,
        None => return Ok(None),
    };

    let truck_id = match extract_reference_value(&player_block, "my_truck") {
        Some(value) => value,
        None => return Ok(None),
    };

    let vehicle_block = match extract_named_block(content, "vehicle", &truck_id)? {
        Some(value) => value,
        None => return Ok(None),
    };

    let odometer_km = extract_integer_field(&vehicle_block, "odometer");
    let cleaned_plate = extract_quoted_field(&vehicle_block, "license_plate")
        .and_then(|plate| sanitize_license_plate(&plate));

    let accessory_ids = extract_array_references(&vehicle_block, "accessories")?;
    let (brand_token, model_token) = resolve_truck_tokens_from_accessories(content, &accessory_ids)?;
    let brand_label = brand_token
        .as_deref()
        .map(humanize_vehicle_token)
        .filter(|value| !value.is_empty());
    let model_label = model_token
        .as_deref()
        .map(humanize_vehicle_token)
        .filter(|value| !value.is_empty());
    let display_name = build_display_name(brand_label.as_deref(), model_label.as_deref());

    if brand_label.is_none()
        && model_label.is_none()
        && display_name.is_none()
        && odometer_km.is_none()
        && cleaned_plate.is_none()
    {
        return Ok(None);
    }

    Ok(Some(CurrentTruckSummary {
        brand_label,
        model_label,
        display_name,
        odometer_km,
        cleaned_plate,
    }))
}

fn extract_first_block(content: &str, block_type: &str) -> Result<Option<(String, String)>, String> {
    let pattern = format!(
        r"\b{}\b\s*:\s*([A-Za-z0-9._]+)\s*\{{",
        regex::escape(block_type)
    );
    let regex = cragex(&pattern)?;
    let captures = match regex.captures(content) {
        Some(captures) => captures,
        None => return Ok(None),
    };

    let id = captures
        .get(1)
        .map(|value| value.as_str().trim().to_string())
        .unwrap_or_default();
    let body = extract_body_from_match(content, captures.get(0).map(|value| value.start()).unwrap_or(0))
        .ok_or_else(|| format!("Failed to read {} block", block_type))?;

    Ok(Some((id, body)))
}

fn extract_named_block(
    content: &str,
    block_type: &str,
    id: &str,
) -> Result<Option<String>, String> {
    let pattern = format!(
        r"\b{}\b\s*:\s*{}\s*\{{",
        regex::escape(block_type),
        regex::escape(id)
    );
    let regex = cragex(&pattern)?;
    let matched = match regex.find(content) {
        Some(matched) => matched,
        None => return Ok(None),
    };

    Ok(extract_body_from_match(content, matched.start()))
}

fn extract_body_from_match(content: &str, match_start: usize) -> Option<String> {
    let relative_brace = content[match_start..].find('{')?;
    let brace_start = match_start + relative_brace;
    let mut depth = 0i32;

    for (offset, character) in content[brace_start..].char_indices() {
        if character == '{' {
            depth += 1;
        } else if character == '}' {
            depth -= 1;
            if depth == 0 {
                let body_start = brace_start + 1;
                let body_end = brace_start + offset;
                return Some(content[body_start..body_end].to_string());
            }
        }
    }

    None
}

fn extract_reference_value(block: &str, key: &str) -> Option<String> {
    let regex = cragex(&format!(
        r"\b{}\b\s*:\s*([A-Za-z0-9._]+|null)",
        regex::escape(key)
    ))
    .ok()?;
    regex.captures(block).and_then(|captures| {
        let value = captures.get(1)?.as_str().trim();
        if value.eq_ignore_ascii_case("null") || value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn extract_integer_field(block: &str, key: &str) -> Option<i64> {
    let regex = cragex(&format!(r"\b{}\b\s*:\s*(\d+)", regex::escape(key))).ok()?;
    regex
        .captures(block)
        .and_then(|captures| captures.get(1)?.as_str().parse::<i64>().ok())
}

fn extract_quoted_field(block: &str, key: &str) -> Option<String> {
    let regex = cragex(&format!(r#"\b{}\b\s*:\s*"([^"]*)""#, regex::escape(key))).ok()?;
    regex
        .captures(block)
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()))
}

fn extract_array_references(block: &str, key: &str) -> Result<Vec<String>, String> {
    let regex = cragex(&format!(r"\b{}\b\[\d+\]:\s*([^\s]+)", regex::escape(key)))?;
    Ok(regex
        .captures_iter(block)
        .filter_map(|captures| captures.get(1).map(|value| value.as_str().trim().to_string()))
        .filter(|value| !value.is_empty())
        .collect())
}

fn resolve_truck_tokens_from_accessories(
    content: &str,
    accessory_ids: &[String],
) -> Result<(Option<String>, Option<String>), String> {
    for accessory_id in accessory_ids {
        let Some(accessory_block) = extract_named_block(content, "vehicle_accessory", accessory_id)? else {
            continue;
        };

        let Some(data_path) = extract_quoted_field(&accessory_block, "data_path") else {
            continue;
        };

        if let Some((brand, model)) = parse_truck_tokens_from_data_path(&data_path) {
            return Ok((brand, model));
        }
    }

    Ok((None, None))
}

fn parse_truck_tokens_from_data_path(data_path: &str) -> Option<(Option<String>, Option<String>)> {
    let marker = "/def/vehicle/truck/";
    let path = data_path.trim();
    let start = path.find(marker)?;
    let truck_key = path[start + marker.len()..].split('/').next()?.trim();

    if truck_key.is_empty() {
        return None;
    }

    let cleaned_key = truck_key.trim_end_matches(".sii");
    if let Some((brand, model)) = cleaned_key.split_once('.') {
        return Some((
            non_empty_option(brand.to_string()),
            non_empty_option(model.to_string()),
        ));
    }

    Some((non_empty_option(cleaned_key.to_string()), None))
}

fn sanitize_license_plate(raw: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }

    let without_tags = cragex(r"<[^>]*>")
        .ok()?
        .replace_all(raw, " ")
        .to_string();
    let without_country = cragex(r"\s*\|[A-Za-z0-9._-]+$")
        .ok()?
        .replace(&without_tags, "")
        .to_string();
    let without_controls = cragex(r"[\r\n\t]+")
        .ok()?
        .replace_all(&without_country, " ")
        .to_string();
    let collapsed = cragex(r"\s+")
        .ok()?
        .replace_all(&without_controls, " ")
        .to_string();

    let cleaned = collapsed
        .trim()
        .trim_matches(|character: char| character == '.' || character == '|' || character == '"')
        .trim()
        .to_string();

    if cleaned.is_empty()
        || cleaned.contains('<')
        || cleaned.contains('>')
        || cleaned.contains("/material/")
    {
        None
    } else {
        Some(cleaned)
    }
}

fn build_display_name(brand_label: Option<&str>, model_label: Option<&str>) -> Option<String> {
    match (brand_label, model_label) {
        (Some(brand), Some(model)) => Some(format!("{} {}", brand, model)),
        (Some(brand), None) => Some(brand.to_string()),
        (None, Some(model)) => Some(model.to_string()),
        (None, None) => None,
    }
}

fn humanize_vehicle_token(raw: &str) -> String {
    raw.split(['.', '_', '-', '/'])
        .flat_map(split_alpha_numeric_segments)
        .filter(|segment| !segment.trim().is_empty())
        .map(|segment| format_vehicle_segment(&segment))
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_alpha_numeric_segments(segment: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut previous_kind: Option<u8> = None;

    for character in segment.chars() {
        let current_kind = if character.is_ascii_alphabetic() {
            0
        } else if character.is_ascii_digit() {
            1
        } else {
            2
        };

        if !current.is_empty()
            && matches!(previous_kind, Some(previous) if previous != current_kind && previous < 2 && current_kind < 2)
        {
            parts.push(current.clone());
            current.clear();
        }

        current.push(character);
        previous_kind = Some(current_kind);
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn format_vehicle_segment(segment: &str) -> String {
    let lower = segment.trim().to_lowercase();
    if lower.is_empty() {
        return String::new();
    }

    if lower.chars().all(|character| character.is_ascii_digit()) {
        return lower;
    }

    if lower.len() <= 3 {
        return lower.to_uppercase();
    }

    let mut characters = lower.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

fn non_empty_option(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn build_quicksave_game_info_from_path(
    path: PathBuf,
    decrypt_cache: &DecryptCache,
) -> Result<(GameDataQuicksave, Option<CurrentTruckSummary>), String> {
    let content = decrypt_cached_with_cache(&path, decrypt_cache).map_err(|error| {
        dev_log!("quicksave_game_info failed: {}", error);
        error
    })?;
    let mut parser_trace = TraceScope::with_fields(
        "quicksave_game_info parser",
        &[("path", path.display().to_string())],
    );

    let (player_id, player_block) = extract_first_block(&content, "player")?
        .ok_or_else(|| "Player block not found".to_string())?;
    let player_my_truck = extract_reference_value(&player_block, "my_truck");
    let player_my_trailer = extract_reference_value(&player_block, "my_trailer");
    let player_xp = extract_integer_field(&content, "experience_points");
    let bank_id = extract_first_block(&content, "bank")?.map(|(id, _)| id);
    let skill = |name: &str| -> Option<i64> { extract_integer_field(&content, name) };
    let truck_summary = parse_current_truck_summary_from_content(&content)?;

    let result = GameDataQuicksave {
        player_id: Some(player_id),
        bank_id,
        player_xp,
        player_my_truck: player_my_truck.clone(),
        player_my_trailer,
        adr: skill("adr"),
        long_dist: skill("long_dist"),
        heavy: skill("heavy"),
        fragile: skill("fragile"),
        urgent: skill("urgent"),
        mechanical: skill("mechanical"),
        vehicle_id: player_my_truck,
        brand_path: None,
        license_plate: truck_summary
            .as_ref()
            .and_then(|summary| summary.cleaned_plate.clone()),
        odometer: truck_summary
            .as_ref()
            .and_then(|summary| summary.odometer_km),
        trip_fuel_l: None,
        truck_brand: None,
        truck_model: None,
        truck_brand_label: truck_summary
            .as_ref()
            .and_then(|summary| summary.brand_label.clone()),
        truck_model_label: truck_summary
            .as_ref()
            .and_then(|summary| summary.model_label.clone()),
        truck_display_name: truck_summary
            .as_ref()
            .and_then(|summary| summary.display_name.clone()),
        trailer_brand: None,
        trailer_model: None,
        trailer_license_plate: None,
        trailer_odometer: None,
        trailer_odometer_float: None,
        trailer_wear_float: None,
        trailer_wheels_float: None,
    };

    parser_trace.finish_ok();
    Ok((result, truck_summary))
}

fn build_current_truck_summary_from_path(
    path: PathBuf,
    decrypt_cache: &DecryptCache,
) -> Result<Option<CurrentTruckSummary>, String> {
    let content = decrypt_cached_with_cache(&path, decrypt_cache).map_err(|error| {
        dev_log!("truck_summary failed: {}", error);
        error
    })?;

    parse_current_truck_summary_from_content(&content).map_err(|error| {
        dev_log!("truck_summary failed: {}", error);
        error
    })
}
