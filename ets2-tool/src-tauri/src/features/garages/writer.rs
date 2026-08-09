use std::collections::{BTreeMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::features::ets2save::sii_codec::replace_file_atomic;
use crate::features::truck_change::parser::{
    UnitBlock, extract_array_entries, extract_field_value, is_null_ref, normalize_sii_unit_id,
    parse_unit_blocks,
};

use super::parser::city_token_from_garage_id;

const LINE_FEED: char = 10 as char;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GarageWritePlan {
    pub content: String,
    pub changed_unit_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GarageResourceAssignmentOptions {
    pub assign_random_driver: bool,
    pub assign_random_truck: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GarageResourceAssignmentWritePlan {
    pub content: String,
    pub changed_unit_ids: Vec<String>,
    pub assigned_driver_id: Option<String>,
    pub assigned_truck_id: Option<String>,
    pub assigned_driver_slot_index: Option<usize>,
    pub assigned_truck_slot_index: Option<usize>,
}

pub fn apply_garage_changes(
    content: &str,
    garage_id: &str,
    target_status_and_capacity: Option<(i32, usize)>,
    set_as_headquarters: bool,
) -> Result<GarageWritePlan, String> {
    let mut updated = content.to_string();
    let mut changed_unit_ids = Vec::new();

    if let Some((target_status, target_capacity)) = target_status_and_capacity {
        updated = resize_garage_capacity(&updated, garage_id, target_status, target_capacity)?;
        changed_unit_ids.push(garage_id.to_string());
    }

    if set_as_headquarters {
        let (next_content, player_id) = set_headquarters(&updated, garage_id)?;
        updated = next_content;
        changed_unit_ids.push(player_id);
    }

    changed_unit_ids.sort();
    changed_unit_ids.dedup();
    Ok(GarageWritePlan {
        content: updated,
        changed_unit_ids,
    })
}

pub fn apply_garage_purchase_batch(
    content: &str,
    garage_ids: &[String],
) -> Result<GarageWritePlan, String> {
    let unit_blocks = parse_unit_blocks(content);
    let mut replacements = Vec::with_capacity(garage_ids.len());
    let mut seen = HashSet::with_capacity(garage_ids.len());

    for garage_id in garage_ids {
        if !seen.insert(garage_id.as_str()) {
            return Err(format!("garage_reference_ambiguous:{garage_id}"));
        }
        let block = unique_unit_block_from_blocks(&unit_blocks, "garage", garage_id)?;
        validate_reusable_profit_log_in_blocks(&unit_blocks, &block)?;
        let rewritten_block = rewrite_garage_capacity(&block.raw_block, 3, 5)?;
        replacements.push((block, rewritten_block));
    }

    replacements.sort_by(|(left, _), (right, _)| right.start_line.cmp(&left.start_line));
    let mut updated = content.to_string();
    for (block, rewritten_block) in replacements {
        updated = replace_unit_block(&updated, &block, &rewritten_block)?;
    }
    let mut changed_unit_ids = garage_ids.to_vec();
    changed_unit_ids.sort();
    Ok(GarageWritePlan {
        content: updated,
        changed_unit_ids,
    })
}

pub fn apply_garage_relinquishment(
    content: &str,
    garage_id: &str,
) -> Result<GarageWritePlan, String> {
    let plan = apply_garage_relinquishment_batch(content, &[garage_id.to_string()])?;
    Ok(plan)
}

pub fn apply_garage_relinquishment_batch(
    content: &str,
    garage_ids: &[String],
) -> Result<GarageWritePlan, String> {
    let unit_blocks = parse_unit_blocks(content);
    let mut replacements = Vec::with_capacity(garage_ids.len());
    let mut seen = HashSet::with_capacity(garage_ids.len());

    for garage_id in garage_ids {
        if !seen.insert(garage_id.as_str()) {
            return Err(format!("garage_reference_ambiguous:{garage_id}"));
        }
        let block = unique_unit_block_from_blocks(&unit_blocks, "garage", garage_id)?;
        validate_reusable_profit_log_in_blocks(&unit_blocks, &block)?;
        ensure_garage_empty_for_relinquishment(&block)?;
        let resized = rewrite_garage_capacity(&block.raw_block, 0, 0)?;
        let rewritten_block = replace_scalar_field(&resized, "productivity", "0")?;
        replacements.push((block, rewritten_block));
    }

    replacements.sort_by(|(left, _), (right, _)| right.start_line.cmp(&left.start_line));
    let mut updated = content.to_string();
    for (block, rewritten_block) in replacements {
        updated = replace_unit_block(&updated, &block, &rewritten_block)?;
    }
    let mut changed_unit_ids = garage_ids.to_vec();
    changed_unit_ids.sort();
    Ok(GarageWritePlan {
        content: updated,
        changed_unit_ids,
    })
}

pub fn apply_random_resource_assignment(
    content: &str,
    garage_id: &str,
    options: GarageResourceAssignmentOptions,
) -> Result<GarageResourceAssignmentWritePlan, String> {
    if !options.assign_random_driver && !options.assign_random_truck {
        return Err("garage_assignment_empty".to_string());
    }

    let unit_blocks = parse_unit_blocks(content);
    let garage_block = unique_unit_block_from_blocks(&unit_blocks, "garage", garage_id)?;
    let vehicle_count = parse_array_count(&garage_block, "vehicles")?;
    let driver_count = parse_array_count(&garage_block, "drivers")?;
    let mut vehicles = array_map(&garage_block.raw_block, "vehicles", vehicle_count)?;
    let mut drivers = array_map(&garage_block.raw_block, "drivers", driver_count)?;
    let mut garage_raw = garage_block.raw_block.clone();
    let mut replacements = Vec::new();
    let mut changed_unit_ids = vec![garage_id.to_string()];
    let mut assigned_truck_id = None;
    let mut assigned_truck_slot_index = None;
    let mut assigned_driver_id = None;
    let mut assigned_driver_slot_index = None;

    if options.assign_random_truck {
        let truck_slots = (0..vehicle_count)
            .filter(|index| vehicles.get(index).is_none_or(|value| is_null_ref(value)))
            .collect::<Vec<_>>();
        if truck_slots.is_empty() {
            return Err("garage_assignment_no_free_vehicle_slot".to_string());
        }
        let available_trucks = available_owned_truck_ids(&unit_blocks)?;
        if available_trucks.is_empty() {
            return Err("garage_assignment_no_available_truck".to_string());
        }
        let slot_index = choose_random_usize(&truck_slots);
        let truck_id = choose_random_string(&available_trucks);
        garage_raw = replace_array_value(&garage_raw, "vehicles", slot_index, &truck_id)?;
        vehicles.insert(slot_index, truck_id.clone());
        assigned_truck_id = Some(truck_id);
        assigned_truck_slot_index = Some(slot_index);
    }

    if options.assign_random_driver {
        let driver_slots = (0..driver_count)
            .filter(|index| drivers.get(index).is_none_or(|value| is_null_ref(value)))
            .filter(|index| vehicles.get(index).is_some_and(|value| !is_null_ref(value)))
            .collect::<Vec<_>>();
        if driver_slots.is_empty() {
            return Err("garage_assignment_no_free_driver_slot".to_string());
        }
        let available_drivers = available_ai_driver_ids(&unit_blocks)?;
        if available_drivers.is_empty() {
            return Err("garage_assignment_no_available_driver".to_string());
        }
        let slot_index = choose_random_usize(&driver_slots);
        let driver_id = choose_random_string(&available_drivers);
        let truck_id = vehicles
            .get(&slot_index)
            .filter(|value| !is_null_ref(value))
            .cloned()
            .ok_or_else(|| "garage_assignment_no_free_driver_slot".to_string())?;
        let driver_block = unique_unit_block_from_blocks(&unit_blocks, "driver_ai", &driver_id)?;
        garage_raw = replace_array_value(&garage_raw, "drivers", slot_index, &driver_id)?;
        let rewritten_driver_block =
            replace_scalar_field(&driver_block.raw_block, "assigned_truck", &truck_id)?;
        replacements.push((driver_block, rewritten_driver_block));
        changed_unit_ids.push(driver_id.clone());
        assigned_driver_id = Some(driver_id);
        assigned_driver_slot_index = Some(slot_index);
    }

    replacements.push((garage_block, garage_raw));
    replacements.sort_by(|(left, _), (right, _)| right.start_line.cmp(&left.start_line));
    let mut updated = content.to_string();
    for (block, rewritten_block) in replacements {
        updated = replace_unit_block(&updated, &block, &rewritten_block)?;
    }
    changed_unit_ids.sort();
    changed_unit_ids.dedup();

    Ok(GarageResourceAssignmentWritePlan {
        content: updated,
        changed_unit_ids,
        assigned_driver_id,
        assigned_truck_id,
        assigned_driver_slot_index,
        assigned_truck_slot_index,
    })
}

pub fn write_verified_content(
    target_path: &Path,
    content: &str,
    verify_candidate: impl Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    verify_candidate(content)?;
    let temporary_path = temporary_path_for(target_path);

    let write_result = (|| {
        let mut temporary_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|_| "save_write_failed:temporary_create".to_string())?;
        temporary_file
            .write_all(content.as_bytes())
            .map_err(|_| "save_write_failed:temporary_write".to_string())?;
        temporary_file
            .flush()
            .map_err(|_| "save_write_failed:temporary_flush".to_string())?;
        temporary_file
            .sync_all()
            .map_err(|_| "save_write_failed:temporary_sync".to_string())?;
        drop(temporary_file);
        let temporary_content = fs::read_to_string(&temporary_path)
            .map_err(|_| "save_write_failed:temporary_readback".to_string())?;
        verify_candidate(&temporary_content)?;
        replace_file_atomic(&temporary_path, target_path)
            .map_err(|_| "save_write_failed:atomic_replace".to_string())
    })();

    if write_result.is_err() && temporary_path.exists() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

fn parse_array_count(block: &UnitBlock, field: &str) -> Result<usize, String> {
    extract_field_value(&block.raw_block, field)
        .ok_or_else(|| format!("garage_block_invalid:{}:{field}_missing", block.id))?
        .parse::<usize>()
        .map_err(|_| format!("garage_block_invalid:{}:{field}_invalid", block.id))
}

fn array_map(
    raw_block: &str,
    field: &str,
    expected_count: usize,
) -> Result<BTreeMap<usize, String>, String> {
    let entries = extract_array_entries(raw_block, field);
    let map = entries.iter().cloned().collect::<BTreeMap<_, _>>();
    if entries.len() != expected_count
        || map.len() != expected_count
        || (0..expected_count).any(|index| !map.contains_key(&index))
    {
        return Err(format!("garage_block_invalid:{field}_indices_invalid"));
    }
    Ok(map)
}

fn available_owned_truck_ids(unit_blocks: &[UnitBlock]) -> Result<Vec<String>, String> {
    let Some(player_block) = player_block_from_blocks(unit_blocks)? else {
        return Err("garage_block_invalid:player_missing".to_string());
    };
    let used_trucks = used_truck_ids(unit_blocks);
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for (_, truck_id) in extract_array_entries(&player_block.raw_block, "trucks") {
        if is_null_ref(&truck_id) {
            continue;
        }
        let normalized = normalize_sii_unit_id(&truck_id);
        if normalized.is_empty()
            || used_trucks.contains(&normalized)
            || !seen.insert(normalized.clone())
        {
            continue;
        }
        let Some(block) = optional_unique_unit_block(unit_blocks, "vehicle", &truck_id)? else {
            continue;
        };
        candidates.push(block.id);
    }
    candidates.sort();
    Ok(candidates)
}

fn available_ai_driver_ids(unit_blocks: &[UnitBlock]) -> Result<Vec<String>, String> {
    let used_drivers = used_driver_ids(unit_blocks);
    let mut candidates = Vec::new();
    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "driver_ai")
    {
        let normalized = normalize_sii_unit_id(&block.id);
        if normalized.is_empty() || used_drivers.contains(&normalized) {
            continue;
        }
        if extract_field_value(&block.raw_block, "assigned_truck")
            .is_some_and(|value| !is_null_ref(&value))
        {
            continue;
        }
        candidates.push(block.id.clone());
    }
    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

fn used_truck_ids(unit_blocks: &[UnitBlock]) -> HashSet<String> {
    let mut used = HashSet::new();
    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "garage")
    {
        for (_, truck_id) in extract_array_entries(&block.raw_block, "vehicles") {
            if !is_null_ref(&truck_id) {
                used.insert(normalize_sii_unit_id(&truck_id));
            }
        }
    }
    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "driver_ai")
    {
        if let Some(truck_id) = extract_field_value(&block.raw_block, "assigned_truck") {
            if !is_null_ref(&truck_id) {
                used.insert(normalize_sii_unit_id(&truck_id));
            }
        }
    }
    if let Ok(Some(player_block)) = player_block_from_blocks(unit_blocks) {
        for field in ["assigned_truck", "my_truck"] {
            if let Some(truck_id) = extract_field_value(&player_block.raw_block, field) {
                if !is_null_ref(&truck_id) {
                    used.insert(normalize_sii_unit_id(&truck_id));
                }
            }
        }
    }
    used.retain(|value| !value.is_empty());
    used
}

fn used_driver_ids(unit_blocks: &[UnitBlock]) -> HashSet<String> {
    let mut used = HashSet::new();
    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "garage")
    {
        for (_, driver_id) in extract_array_entries(&block.raw_block, "drivers") {
            if !is_null_ref(&driver_id) {
                used.insert(normalize_sii_unit_id(&driver_id));
            }
        }
    }
    for block in unit_blocks
        .iter()
        .filter(|block| block.unit_type == "driver_ai")
    {
        if extract_field_value(&block.raw_block, "assigned_truck")
            .is_some_and(|truck_id| !is_null_ref(&truck_id))
        {
            used.insert(normalize_sii_unit_id(&block.id));
        }
    }
    used.retain(|value| !value.is_empty());
    used
}

fn player_block_from_blocks(unit_blocks: &[UnitBlock]) -> Result<Option<UnitBlock>, String> {
    let economy_blocks = unit_blocks
        .iter()
        .filter(|block| block.unit_type == "economy")
        .collect::<Vec<_>>();
    let economy_block = match economy_blocks.as_slice() {
        [] => return Ok(None),
        [block] => *block,
        _ => return Err("garage_reference_ambiguous:economy".to_string()),
    };
    let player_id = extract_field_value(&economy_block.raw_block, "player")
        .ok_or_else(|| "garage_block_invalid:player_reference_missing".to_string())?;
    optional_unique_unit_block(unit_blocks, "player", &player_id)
}

fn optional_unique_unit_block(
    unit_blocks: &[UnitBlock],
    unit_type: &str,
    unit_id: &str,
) -> Result<Option<UnitBlock>, String> {
    let matching = unit_blocks
        .iter()
        .filter(|block| block.unit_type == unit_type && block.id.eq_ignore_ascii_case(unit_id))
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [] => Ok(None),
        [block] => Ok(Some((**block).clone())),
        _ => Err(format!("garage_reference_ambiguous:{unit_id}")),
    }
}

fn choose_random_usize(values: &[usize]) -> usize {
    values[(Uuid::new_v4().as_u128() as usize) % values.len()]
}

fn choose_random_string(values: &[String]) -> String {
    values[(Uuid::new_v4().as_u128() as usize) % values.len()].clone()
}

fn resize_garage_capacity(
    content: &str,
    garage_id: &str,
    target_status: i32,
    target_capacity: usize,
) -> Result<String, String> {
    let block = unique_unit_block(content, "garage", garage_id)?;
    validate_reusable_profit_log(content, &block)?;
    let rewritten_block =
        rewrite_garage_capacity(&block.raw_block, target_status, target_capacity)?;
    replace_unit_block(content, &block, &rewritten_block)
}

fn rewrite_garage_capacity(
    raw_block: &str,
    target_status: i32,
    target_capacity: usize,
) -> Result<String, String> {
    let with_vehicles = resize_array_field(raw_block, "vehicles", target_capacity)?;
    let with_drivers = resize_array_field(&with_vehicles, "drivers", target_capacity)?;
    replace_scalar_field(&with_drivers, "status", &target_status.to_string())
}

fn validate_reusable_profit_log(content: &str, garage_block: &UnitBlock) -> Result<(), String> {
    validate_reusable_profit_log_in_blocks(&parse_unit_blocks(content), garage_block)
}

fn validate_reusable_profit_log_in_blocks(
    unit_blocks: &[UnitBlock],
    garage_block: &UnitBlock,
) -> Result<(), String> {
    let profit_log_id = extract_field_value(&garage_block.raw_block, "profit_log")
        .filter(|value| !is_null_ref(value))
        .ok_or_else(|| format!("garage_profit_log_reference_unresolved:{}", garage_block.id))?;
    let matching = unit_blocks
        .iter()
        .filter(|block| block.id == profit_log_id)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [block] if block.unit_type == "profit_log" => Ok(()),
        [] | [_] => Err(format!(
            "garage_profit_log_reference_unresolved:{}",
            garage_block.id
        )),
        _ => Err(format!(
            "garage_profit_log_reference_ambiguous:{}",
            garage_block.id
        )),
    }
}

fn ensure_garage_empty_for_relinquishment(garage_block: &UnitBlock) -> Result<(), String> {
    for field in ["vehicles", "drivers"] {
        if extract_array_entries(&garage_block.raw_block, field)
            .iter()
            .any(|(_, value)| !is_null_ref(value))
        {
            return Err("garage_relinquish_not_empty".to_string());
        }
    }
    let trailer_count = extract_field_value(&garage_block.raw_block, "trailers")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| "garage_block_invalid:trailers_invalid".to_string())?;
    if trailer_count != 0
        || extract_array_entries(&garage_block.raw_block, "trailers")
            .iter()
            .any(|(_, value)| !is_null_ref(value))
    {
        return Err("garage_relinquish_not_empty".to_string());
    }
    Ok(())
}

fn set_headquarters(content: &str, garage_id: &str) -> Result<(String, String), String> {
    let city_token = city_token_from_garage_id(garage_id)
        .ok_or_else(|| "garage_block_invalid:invalid_garage_id".to_string())?;
    let economy_blocks = parse_unit_blocks(content)
        .into_iter()
        .filter(|block| block.unit_type == "economy")
        .collect::<Vec<_>>();
    let economy_block = match economy_blocks.as_slice() {
        [] => return Err("garage_block_invalid:economy_missing".to_string()),
        [block] => block,
        _ => return Err("garage_reference_ambiguous:economy".to_string()),
    };
    let player_id = extract_field_value(&economy_block.raw_block, "player")
        .ok_or_else(|| "garage_block_invalid:player_reference_missing".to_string())?;
    let player_block = unique_unit_block(content, "player", &player_id)?;
    let rewritten_block = replace_scalar_field(&player_block.raw_block, "hq_city", &city_token)?;
    Ok((
        replace_unit_block(content, &player_block, &rewritten_block)?,
        player_id,
    ))
}

fn unique_unit_block(content: &str, unit_type: &str, unit_id: &str) -> Result<UnitBlock, String> {
    unique_unit_block_from_blocks(&parse_unit_blocks(content), unit_type, unit_id)
}

fn unique_unit_block_from_blocks(
    unit_blocks: &[UnitBlock],
    unit_type: &str,
    unit_id: &str,
) -> Result<UnitBlock, String> {
    let matching = unit_blocks
        .iter()
        .filter(|block| block.unit_type == unit_type && block.id == unit_id)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [] if unit_type == "garage" => Err(format!("garage_not_found:{unit_id}")),
        [] => Err(format!("garage_block_invalid:{unit_type}_missing")),
        [block] => Ok((**block).clone()),
        _ => Err(format!("garage_reference_ambiguous:{unit_id}")),
    }
}

fn resize_array_field(raw_block: &str, field: &str, target_count: usize) -> Result<String, String> {
    let mut lines = raw_block.lines().map(str::to_string).collect::<Vec<_>>();
    let scalar_prefix = format!("{field}:");
    let count_lines = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(&scalar_prefix))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let count_index = match count_lines.as_slice() {
        [index] => *index,
        [] => return Err(format!("garage_block_invalid:{field}_missing")),
        _ => return Err(format!("garage_block_invalid:{field}_ambiguous")),
    };
    let current_count = lines[count_index]
        .trim()
        .split_once(':')
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .ok_or_else(|| format!("garage_block_invalid:{field}_invalid"))?;
    let entries = extract_array_entries(raw_block, field);
    let entry_indices = entries
        .iter()
        .map(|(index, _)| *index)
        .collect::<HashSet<_>>();
    if entries.len() != current_count
        || entry_indices.len() != current_count
        || (0..current_count).any(|index| !entry_indices.contains(&index))
    {
        return Err(format!("garage_block_invalid:{field}_indices_invalid"));
    }

    let count_indent = line_indent(&lines[count_index]);
    lines[count_index] = format!("{count_indent}{field}: {target_count}");
    if target_count == current_count {
        return Ok(join_lines(&lines));
    }
    if target_count < current_count {
        if let Some((index, _)) = entries
            .iter()
            .find(|(index, value)| *index >= target_count && !is_null_ref(value))
        {
            return Err(format!(
                "garage_downgrade_capacity_exceeded:{field}:slot={index}"
            ));
        }
        lines.retain(|line| match array_index_for_line(line, field) {
            Some(index) => index < target_count,
            None => true,
        });
        return Ok(join_lines(&lines));
    }

    let array_prefix = format!("{field}[");
    let last_array_index = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(&array_prefix))
        .map(|(index, _)| index)
        .next_back();
    let insert_at = last_array_index.map_or(count_index + 1, |index| index + 1);
    let array_indent = last_array_index
        .map(|index| line_indent(&lines[index]))
        .unwrap_or_else(|| count_indent.clone());
    let new_entries = (current_count..target_count)
        .map(|index| format!("{array_indent}{field}[{index}]: null"))
        .collect::<Vec<_>>();
    lines.splice(insert_at..insert_at, new_entries);
    Ok(join_lines(&lines))
}

fn array_index_for_line(line: &str, field: &str) -> Option<usize> {
    let prefix = format!("{field}[");
    let suffix = line.trim_start().strip_prefix(&prefix)?;
    let (index, _) = suffix.split_once("]:")?;
    index.parse::<usize>().ok()
}

fn replace_array_value(
    raw_block: &str,
    field: &str,
    array_index: usize,
    value: &str,
) -> Result<String, String> {
    let mut lines = raw_block.lines().map(str::to_string).collect::<Vec<_>>();
    let prefix = format!("{field}[{array_index}]:");
    let matching = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(&prefix))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let line_index = match matching.as_slice() {
        [index] => *index,
        [] => return Err(format!("garage_block_invalid:{field}_slot_missing")),
        _ => return Err(format!("garage_block_invalid:{field}_slot_ambiguous")),
    };
    let indent = line_indent(&lines[line_index]);
    lines[line_index] = format!("{indent}{field}[{array_index}]: {value}");
    Ok(join_lines(&lines))
}

fn replace_scalar_field(raw_block: &str, field: &str, value: &str) -> Result<String, String> {
    let mut lines = raw_block.lines().map(str::to_string).collect::<Vec<_>>();
    let prefix = format!("{field}:");
    let matching = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(&prefix))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let index = match matching.as_slice() {
        [index] => *index,
        [] => return Err(format!("garage_block_invalid:{field}_missing")),
        _ => return Err(format!("garage_block_invalid:{field}_ambiguous")),
    };
    let indent = line_indent(&lines[index]);
    lines[index] = format!("{indent}{field}: {value}");
    Ok(join_lines(&lines))
}

fn replace_unit_block(
    content: &str,
    block: &UnitBlock,
    rewritten_block: &str,
) -> Result<String, String> {
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    if block.end_line >= lines.len() || block.start_line > block.end_line {
        return Err("garage_block_invalid:block_range".to_string());
    }
    lines.splice(
        block.start_line..=block.end_line,
        rewritten_block.lines().map(str::to_string),
    );
    let mut updated = join_lines(&lines);
    if content.ends_with(LINE_FEED) {
        updated.push(LINE_FEED);
    }
    Ok(updated)
}

fn join_lines(lines: &[String]) -> String {
    lines.join(&LINE_FEED.to_string())
}

fn line_indent(line: &str) -> String {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .collect()
}

fn temporary_path_for(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("game.sii");
    target_path.with_file_name(format!("{file_name}.garage.{}.tmp", Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        GarageResourceAssignmentOptions, GarageResourceAssignmentWritePlan, apply_garage_changes,
        apply_garage_purchase_batch, apply_garage_relinquishment,
        apply_garage_relinquishment_batch, apply_random_resource_assignment,
        write_verified_content,
    };
    use crate::features::garages::models::{GarageOperation, GarageOwnership, GarageSize};
    use crate::features::garages::parser::parse_garages_from_sii;
    use crate::features::garages::validator::{
        GarageAssignmentVerificationSpec, GarageVerificationSpec, verify_garage_mutation,
        verify_garage_purchase_batch, verify_garage_relinquishment_batch,
        verify_garage_resource_assignment,
    };
    use crate::features::truck_change::parser::parse_unit_blocks;
    use crate::shared::ets2data::validate::sha256_hex_bytes;
    use uuid::Uuid;

    const SAMPLE: &str = include_str!("../../../test-fixtures/garages/garage_samples.sii");
    const REAL_SAMPLE: &str = include_str!("../../../test-fixtures/decrypt/plain_game.sii");
    const PARIS_EMPTY_SMALL: &str = "garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: null\n drivers[1]: null\n drivers[2]: null\n trailers: 0\n status: 2\n profit_log: profit.paris\n productivity: 0\n}";
    const PARIS_WITH_TRUCK: &str = "garage : garage.paris {\n vehicles: 3\n vehicles[0]: truck.paris\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: null\n drivers[1]: null\n drivers[2]: null\n trailers: 0\n status: 2\n profit_log: profit.paris\n productivity: 0\n}";
    const PARIS_FULL: &str = "garage : garage.paris {\n vehicles: 3\n vehicles[0]: truck.paris\n vehicles[1]: truck.free_a\n vehicles[2]: truck.free_b\n drivers: 3\n drivers[0]: driver.free_a\n drivers[1]: driver.free_b\n drivers[2]: driver.free_c\n trailers: 0\n status: 2\n profit_log: profit.paris\n productivity: 0\n}";

    fn sample_with_two_unowned_garages() -> String {
        SAMPLE.replace(
            "garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: null\n drivers[1]: null\n drivers[2]: null\n trailers: 0\n status: 2\n profit_log: profit.paris\n productivity: 0\n}",
            "garage : garage.paris {\n vehicles: 0\n drivers: 0\n trailers: 0\n status: 0\n profit_log: profit.paris\n productivity: 0\n}",
        )
    }

    #[test]
    fn purchase_expands_only_the_selected_garage() {
        let plan = apply_garage_changes(SAMPLE, "garage.los_angeles", Some((3, 5)), false).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let purchased = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.los_angeles")
            .unwrap();
        assert_eq!(purchased.status, Some(3));
        assert_eq!(purchased.vehicle_slot_count, 5);
        assert_eq!(purchased.driver_slot_count, 5);
        assert_eq!(purchased.trailer_slot_count, 0);
        assert!(
            purchased
                .slots
                .iter()
                .all(|slot| { slot.truck_id.is_none() && slot.driver_id.is_none() })
        );
        assert_eq!(
            purchased.profit_log_id.as_deref(),
            Some("profit.los_angeles")
        );
        assert!(
            plan.content
                .contains("future_garage_field: preserved_by_reader")
        );
    }

    #[test]
    fn purchase_batch_reuses_single_garage_writer_for_every_target() {
        let before = sample_with_two_unowned_garages();
        let targets = vec!["garage.paris".to_string(), "garage.los_angeles".to_string()];
        let plan = apply_garage_purchase_batch(&before, &targets).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();

        for garage_id in &targets {
            let garage = parsed
                .garages
                .iter()
                .find(|garage| &garage.garage_id == garage_id)
                .unwrap();
            assert_eq!(garage.status, Some(3));
            assert_eq!(garage.vehicle_slot_count, 5);
            assert_eq!(garage.driver_slot_count, 5);
            assert!(
                garage
                    .slots
                    .iter()
                    .all(|slot| slot.truck_id.is_none() && slot.driver_id.is_none())
            );
        }
        let headquarters = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();
        assert!(headquarters.is_headquarters);
        assert_eq!(headquarters.slots[0].truck_id.as_deref(), Some("truck.one"));
        assert_eq!(
            headquarters.slots[0].driver_id.as_deref(),
            Some("driver.one")
        );
        assert_eq!(headquarters.trailer_ids, vec!["trailer.one"]);
        assert_eq!(
            plan.changed_unit_ids,
            vec!["garage.los_angeles".to_string(), "garage.paris".to_string()]
        );
    }

    #[test]
    fn relinquishment_resets_empty_owned_garage_to_fixture_unowned_shape() {
        let plan = apply_garage_relinquishment(SAMPLE, "garage.paris").unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let relinquished = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap();

        assert_eq!(relinquished.ownership, GarageOwnership::NotOwned);
        assert_eq!(relinquished.size, GarageSize::Unowned);
        assert_eq!(relinquished.status, Some(0));
        assert_eq!(relinquished.vehicle_slot_count, 0);
        assert_eq!(relinquished.driver_slot_count, 0);
        assert_eq!(relinquished.trailer_slot_count, 0);
        assert_eq!(relinquished.productivity, Some(0.0));
        assert_eq!(relinquished.profit_log_id.as_deref(), Some("profit.paris"));
        assert_eq!(
            parsed.headquarters_garage_id.as_deref(),
            Some("garage.berlin")
        );
    }

    #[test]
    fn relinquishment_rejects_truck_driver_or_trailer_references() {
        let truck = SAMPLE.replace(
            "garage : garage.paris {\n vehicles: 3\n vehicles[0]: null",
            "garage : garage.paris {\n vehicles: 3\n vehicles[0]: truck.one",
        );
        assert_eq!(
            apply_garage_relinquishment(&truck, "garage.paris").unwrap_err(),
            "garage_relinquish_not_empty"
        );

        let driver = SAMPLE.replace("garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: null", "garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: driver.one");
        assert_eq!(
            apply_garage_relinquishment(&driver, "garage.paris").unwrap_err(),
            "garage_relinquish_not_empty"
        );

        let trailer = SAMPLE.replace("garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: null\n drivers[1]: null\n drivers[2]: null\n trailers: 0", "garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: null\n drivers[1]: null\n drivers[2]: null\n trailers: 1\n trailers[0]: trailer.one");
        assert_eq!(
            apply_garage_relinquishment(&trailer, "garage.paris").unwrap_err(),
            "garage_relinquish_not_empty"
        );
    }

    #[test]
    fn upgrade_preserves_existing_references() {
        let small = SAMPLE
            .replace("vehicles[0]: null", "vehicles[0]: truck.one")
            .replace("drivers[0]: null", "drivers[0]: driver.one");
        let plan = apply_garage_changes(&small, "garage.paris", Some((3, 5)), false).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let upgraded = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap();
        assert_eq!(upgraded.slots[0].truck_id.as_deref(), Some("truck.one"));
        assert_eq!(upgraded.slots[0].driver_id.as_deref(), Some("driver.one"));
        assert_eq!(upgraded.vehicle_slot_count, 5);
        assert_eq!(upgraded.driver_slot_count, 5);
    }

    #[test]
    fn setting_headquarters_changes_only_player_hq_city() {
        let plan = apply_garage_changes(SAMPLE, "garage.paris", None, true).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        assert_eq!(
            parsed.headquarters_garage_id.as_deref(),
            Some("garage.paris")
        );
        assert_eq!(plan.changed_unit_ids, vec!["_player"]);
    }

    #[test]
    fn safe_shrink_preserves_assignments_and_metadata() {
        let plan = apply_garage_changes(SAMPLE, "garage.berlin", Some((2, 3)), false).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let garage = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();

        assert_eq!(garage.status, Some(2));
        assert_eq!(garage.vehicle_slot_count, 3);
        assert_eq!(garage.driver_slot_count, 3);
        assert_eq!(garage.slots[0].truck_id.as_deref(), Some("truck.one"));
        assert_eq!(garage.slots[1].truck_id.as_deref(), Some("truck.two"));
        assert_eq!(garage.slots[0].driver_id.as_deref(), Some("driver.one"));
        assert_eq!(garage.trailer_ids, vec!["trailer.one"]);
        assert!(
            plan.content
                .contains("future_garage_field: preserved_by_reader")
        );
    }

    #[test]
    fn shrink_rejects_reference_outside_target_capacity() {
        let occupied = SAMPLE.replace("vehicles[4]: null", "vehicles[4]: truck.five");
        let error =
            apply_garage_changes(&occupied, "garage.berlin", Some((2, 3)), false).unwrap_err();
        assert_eq!(error, "garage_downgrade_capacity_exceeded:vehicles:slot=4");
    }

    #[test]
    fn shrink_rejects_driver_outside_target_capacity() {
        let occupied = SAMPLE.replace("drivers[4]: null", "drivers[4]: driver.five");
        let error =
            apply_garage_changes(&occupied, "garage.berlin", Some((2, 3)), false).unwrap_err();
        assert_eq!(error, "garage_downgrade_capacity_exceeded:drivers:slot=4");
    }

    #[test]
    fn mutation_rejects_unknown_garage_id() {
        let error =
            apply_garage_changes(SAMPLE, "garage.missing", Some((2, 3)), false).unwrap_err();
        assert_eq!(error, "garage_not_found:garage.missing");
    }

    #[test]
    fn purchase_rejects_missing_profit_log_block() {
        let invalid = SAMPLE.replace(
            "profit_log : profit.los_angeles",
            "profit_log : profit.detached",
        );
        let error =
            apply_garage_changes(&invalid, "garage.los_angeles", Some((3, 5)), false).unwrap_err();
        assert_eq!(
            error,
            "garage_profit_log_reference_unresolved:garage.los_angeles"
        );
    }

    #[test]
    fn failed_preverification_keeps_original_file_unchanged() {
        let path =
            std::env::temp_dir().join(format!("ets2-garage-writer-test-{}.sii", Uuid::new_v4()));
        fs::write(&path, SAMPLE).unwrap();

        let error = write_verified_content(&path, "invalid candidate", |_| {
            Err("save_verification_failed:test".to_string())
        })
        .unwrap_err();

        assert_eq!(error, "save_verification_failed:test");
        assert_eq!(fs::read_to_string(&path).unwrap(), SAMPLE);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn atomic_write_roundtrip_verifies_anonymized_purchase() {
        let path = std::env::temp_dir().join(format!(
            "ets2-garage-write-roundtrip-{}.sii",
            Uuid::new_v4()
        ));
        fs::write(&path, SAMPLE).unwrap();
        let plan = apply_garage_changes(SAMPLE, "garage.los_angeles", Some((3, 5)), false).unwrap();
        let spec = GarageVerificationSpec {
            operation: GarageOperation::Purchase,
            target_size: Some(GarageSize::Large),
            set_as_headquarters: false,
        };

        write_verified_content(&path, &plan.content, |candidate| {
            verify_garage_mutation(SAMPLE, candidate, "garage.los_angeles", &spec).map(|_| ())
        })
        .unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let verified =
            verify_garage_mutation(SAMPLE, &written, "garage.los_angeles", &spec).unwrap();
        assert_eq!(verified.updated_state.size, GarageSize::Large);
        assert_eq!(verified.updated_state.vehicle_slot_count, 5);
        assert_eq!(verified.updated_state.driver_slot_count, 5);
        assert!(
            verified
                .updated_state
                .slots
                .iter()
                .all(|slot| { slot.truck_id.is_none() && slot.driver_id.is_none() })
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn atomic_write_roundtrip_verifies_relinquishment_after_reload() {
        let path = std::env::temp_dir().join(format!(
            "ets2-garage-relinquish-roundtrip-{}.sii",
            Uuid::new_v4()
        ));
        fs::write(&path, SAMPLE).unwrap();
        let plan = apply_garage_relinquishment(SAMPLE, "garage.paris").unwrap();
        let spec = GarageVerificationSpec {
            operation: GarageOperation::Relinquish,
            target_size: Some(GarageSize::Unowned),
            set_as_headquarters: false,
        };

        write_verified_content(&path, &plan.content, |candidate| {
            verify_garage_mutation(SAMPLE, candidate, "garage.paris", &spec).map(|_| ())
        })
        .unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let verified = verify_garage_mutation(SAMPLE, &written, "garage.paris", &spec).unwrap();
        assert_eq!(verified.updated_state.ownership, GarageOwnership::NotOwned);
        assert_eq!(verified.updated_state.status, Some(0));
        assert_eq!(verified.updated_state.productivity, Some(0.0));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn atomic_write_roundtrip_verifies_upgrade_and_downgrade() {
        let path = std::env::temp_dir().join(format!(
            "ets2-garage-resize-roundtrip-{}.sii",
            Uuid::new_v4()
        ));
        fs::write(&path, SAMPLE).unwrap();

        let upgrade_plan =
            apply_garage_changes(SAMPLE, "garage.paris", Some((3, 5)), false).unwrap();
        let upgrade_spec = GarageVerificationSpec {
            operation: GarageOperation::Upgrade,
            target_size: Some(GarageSize::Large),
            set_as_headquarters: false,
        };
        write_verified_content(&path, &upgrade_plan.content, |candidate| {
            verify_garage_mutation(SAMPLE, candidate, "garage.paris", &upgrade_spec).map(|_| ())
        })
        .unwrap();

        let upgraded = fs::read_to_string(&path).unwrap();
        let verified_upgrade =
            verify_garage_mutation(SAMPLE, &upgraded, "garage.paris", &upgrade_spec).unwrap();
        assert_eq!(verified_upgrade.updated_state.size, GarageSize::Large);

        let downgrade_plan =
            apply_garage_changes(&upgraded, "garage.paris", Some((2, 3)), false).unwrap();
        let downgrade_spec = GarageVerificationSpec {
            operation: GarageOperation::Update,
            target_size: Some(GarageSize::Small),
            set_as_headquarters: false,
        };
        write_verified_content(&path, &downgrade_plan.content, |candidate| {
            verify_garage_mutation(&upgraded, candidate, "garage.paris", &downgrade_spec)
                .map(|_| ())
        })
        .unwrap();

        let downgraded = fs::read_to_string(&path).unwrap();
        let verified_downgrade =
            verify_garage_mutation(&upgraded, &downgraded, "garage.paris", &downgrade_spec)
                .unwrap();
        assert_eq!(verified_downgrade.updated_state.size, GarageSize::Small);
        assert_eq!(verified_downgrade.updated_state.vehicle_slot_count, 3);
        assert_eq!(verified_downgrade.updated_state.driver_slot_count, 3);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sequential_purchases_use_the_reloaded_content_and_create_no_trucks() {
        let mut content = SAMPLE.replace(" garages: 4", " garages: 5").replace(
            " garages[3]: garage.unknown_city",
            " garages[3]: garage.unknown_city\n garages[4]: garage.madrid",
        );
        let insert_at = content.rfind("\n}").unwrap();
        content.insert_str(
            insert_at,
            "\n\ngarage : garage.madrid {\n vehicles: 0\n drivers: 0\n trailers: 0\n status: 0\n profit_log: profit.madrid\n productivity: 0\n}\n\nprofit_log : profit.madrid {\n}",
        );
        let vehicle_blocks_before = parse_unit_blocks(&content)
            .iter()
            .filter(|block| block.unit_type == "vehicle")
            .count();

        let first =
            apply_garage_changes(&content, "garage.los_angeles", Some((3, 5)), false).unwrap();
        let first_hash = sha256_hex_bytes(first.content.as_bytes());
        let second =
            apply_garage_changes(&first.content, "garage.madrid", Some((3, 5)), false).unwrap();
        let second_hash = sha256_hex_bytes(second.content.as_bytes());
        let parsed = parse_garages_from_sii(&second.content).unwrap();

        for garage_id in ["garage.los_angeles", "garage.madrid"] {
            let garage = parsed
                .garages
                .iter()
                .find(|garage| garage.garage_id == garage_id)
                .unwrap();
            assert_eq!(garage.status, Some(3));
            assert_eq!(garage.vehicle_slot_count, 5);
            assert_eq!(garage.driver_slot_count, 5);
            assert_eq!(garage.assigned_truck_count, 0);
            assert_eq!(garage.assigned_driver_count, 0);
        }
        assert_ne!(first_hash, second_hash);
        assert_eq!(
            parse_unit_blocks(&second.content)
                .iter()
                .filter(|block| block.unit_type == "vehicle")
                .count(),
            vehicle_blocks_before
        );
    }

    #[test]
    fn three_real_fixture_purchases_reload_disk_and_create_no_units() {
        let path = std::env::temp_dir().join(format!(
            "ets2-garage-three-purchases-{}.sii",
            Uuid::new_v4()
        ));
        fs::write(&path, REAL_SAMPLE).unwrap();
        let unit_counts_before = save_unit_counts(REAL_SAMPLE);
        let mut previous_hash = sha256_hex_bytes(REAL_SAMPLE.as_bytes());
        let purchased_ids = ["garage.leipzig", "garage.cardiff", "garage.magdeburg"];

        for garage_id in purchased_ids {
            let before = fs::read_to_string(&path).unwrap();
            assert_eq!(sha256_hex_bytes(before.as_bytes()), previous_hash);
            let plan = apply_garage_changes(&before, garage_id, Some((3, 5)), false).unwrap();
            let spec = GarageVerificationSpec {
                operation: GarageOperation::Purchase,
                target_size: Some(GarageSize::Large),
                set_as_headquarters: false,
            };
            write_verified_content(&path, &plan.content, |candidate| {
                verify_garage_mutation(&before, candidate, garage_id, &spec).map(|_| ())
            })
            .unwrap();

            let after = fs::read_to_string(&path).unwrap();
            let verified = verify_garage_mutation(&before, &after, garage_id, &spec).unwrap();
            assert_eq!(verified.updated_state.status, Some(3));
            assert_eq!(verified.updated_state.vehicle_slot_count, 5);
            assert_eq!(verified.updated_state.driver_slot_count, 5);
            assert_eq!(verified.updated_state.assigned_truck_count, 0);
            assert_eq!(verified.updated_state.assigned_driver_count, 0);
            previous_hash = sha256_hex_bytes(after.as_bytes());
        }

        let final_content = fs::read_to_string(&path).unwrap();
        let parsed = parse_garages_from_sii(&final_content).unwrap();
        for garage_id in purchased_ids {
            let garage = parsed
                .garages
                .iter()
                .find(|garage| garage.garage_id == garage_id)
                .unwrap();
            assert_eq!(garage.status, Some(3));
            assert_eq!(garage.vehicle_slot_count, 5);
            assert_eq!(garage.driver_slot_count, 5);
        }
        assert_eq!(save_unit_counts(&final_content), unit_counts_before);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn real_fixture_batch_purchases_every_unowned_garage() {
        let before = parse_garages_from_sii(REAL_SAMPLE).unwrap();
        let garage_ids = before
            .garages
            .iter()
            .filter(|garage| garage.ownership == GarageOwnership::NotOwned)
            .map(|garage| garage.garage_id.clone())
            .collect::<Vec<_>>();
        assert!(garage_ids.len() > 100);
        let headquarters_before = before.headquarters_garage_id.clone();
        let unit_counts_before = save_unit_counts(REAL_SAMPLE);

        let plan = apply_garage_purchase_batch(REAL_SAMPLE, &garage_ids).unwrap();
        let verified =
            verify_garage_purchase_batch(REAL_SAMPLE, &plan.content, &garage_ids).unwrap();
        let after = parse_garages_from_sii(&plan.content).unwrap();

        assert_eq!(verified.updated_states.len(), garage_ids.len());
        assert_eq!(after.headquarters_garage_id, headquarters_before);
        assert_eq!(after.diagnostics.not_owned_garage_count, 0);
        assert_eq!(save_unit_counts(&plan.content), unit_counts_before);
    }

    #[test]
    fn real_fixture_upgrade_and_downgrade_preserve_existing_slots() {
        let parsed = parse_garages_from_sii(REAL_SAMPLE).unwrap();
        let previous = parsed
            .garages
            .iter()
            .find(|garage| garage.size == GarageSize::Small)
            .unwrap();
        let garage_id = previous.garage_id.clone();

        let upgrade_plan =
            apply_garage_changes(REAL_SAMPLE, &garage_id, Some((3, 5)), false).unwrap();
        let upgrade_spec = GarageVerificationSpec {
            operation: GarageOperation::Upgrade,
            target_size: Some(GarageSize::Large),
            set_as_headquarters: false,
        };
        let upgraded = verify_garage_mutation(
            REAL_SAMPLE,
            &upgrade_plan.content,
            &garage_id,
            &upgrade_spec,
        )
        .unwrap();
        assert_eq!(&upgraded.updated_state.slots[..3], &previous.slots[..3]);
        assert!(upgraded.updated_state.slots[3].truck_id.is_none());
        assert!(upgraded.updated_state.slots[3].driver_id.is_none());
        assert!(upgraded.updated_state.slots[4].truck_id.is_none());
        assert!(upgraded.updated_state.slots[4].driver_id.is_none());

        let downgrade_plan =
            apply_garage_changes(&upgrade_plan.content, &garage_id, Some((2, 3)), false).unwrap();
        let downgrade_spec = GarageVerificationSpec {
            operation: GarageOperation::Update,
            target_size: Some(GarageSize::Small),
            set_as_headquarters: false,
        };
        let downgraded = verify_garage_mutation(
            &upgrade_plan.content,
            &downgrade_plan.content,
            &garage_id,
            &downgrade_spec,
        )
        .unwrap();
        assert_eq!(downgraded.updated_state.slots, previous.slots);
    }

    #[test]
    fn real_fixture_blocks_downgrade_with_occupied_removed_slot() {
        let error =
            apply_garage_changes(REAL_SAMPLE, "garage.lille", Some((2, 3)), false).unwrap_err();
        assert!(error.starts_with("garage_downgrade_capacity_exceeded:vehicles:slot="));
    }

    fn assignment_sample() -> String {
        let mut content = SAMPLE
            .replace(
                "player : _player {\n hq_city: berlin\n}",
                "player : _player {\n hq_city: berlin\n assigned_truck: null\n my_truck: null\n trucks: 6\n trucks[0]: truck.one\n trucks[1]: truck.two\n trucks[2]: truck.paris\n trucks[3]: truck.free_a\n trucks[4]: truck.free_b\n trucks[5]: truck.free_c\n}",
            )
            .replace("driver_ai : driver.one {\n}", "driver_ai : driver.one {\n assigned_truck: truck.one\n}")
            .replace(PARIS_EMPTY_SMALL, PARIS_WITH_TRUCK);
        let insert_at = content.rfind("\n}").unwrap();
        content.insert_str(
            insert_at,
            "\n\nvehicle : truck.paris {\n}\n\nvehicle : truck.free_a {\n}\n\nvehicle : truck.free_b {\n}\n\nvehicle : truck.free_c {\n}\n\ndriver_ai : driver.free_a {\n assigned_truck: null\n}\n\ndriver_ai : driver.free_b {\n assigned_truck: null\n}\n\ndriver_ai : driver.free_c {\n assigned_truck: null\n}",
        );
        content
    }

    fn assignment_sample_without_paris_truck() -> String {
        assignment_sample().replace(PARIS_WITH_TRUCK, PARIS_EMPTY_SMALL)
    }

    fn assignment_sample_without_available_trucks() -> String {
        assignment_sample().replace(
            " trucks: 6\n trucks[0]: truck.one\n trucks[1]: truck.two\n trucks[2]: truck.paris\n trucks[3]: truck.free_a\n trucks[4]: truck.free_b\n trucks[5]: truck.free_c",
            " trucks: 3\n trucks[0]: truck.one\n trucks[1]: truck.two\n trucks[2]: truck.paris",
        )
    }

    fn assignment_sample_without_available_drivers() -> String {
        assignment_sample()
            .replace(
                "driver_ai : driver.free_a {\n assigned_truck: null\n}",
                "driver_ai : driver.free_a {\n assigned_truck: truck.free_a\n}",
            )
            .replace(
                "driver_ai : driver.free_b {\n assigned_truck: null\n}",
                "driver_ai : driver.free_b {\n assigned_truck: truck.free_b\n}",
            )
            .replace(
                "driver_ai : driver.free_c {\n assigned_truck: null\n}",
                "driver_ai : driver.free_c {\n assigned_truck: truck.free_c\n}",
            )
    }

    fn assignment_sample_with_full_paris() -> String {
        assignment_sample().replace(PARIS_WITH_TRUCK, PARIS_FULL)
    }

    fn assignment_options(
        assign_random_driver: bool,
        assign_random_truck: bool,
    ) -> GarageResourceAssignmentOptions {
        GarageResourceAssignmentOptions {
            assign_random_driver,
            assign_random_truck,
        }
    }

    fn assignment_spec(
        plan: &GarageResourceAssignmentWritePlan,
    ) -> GarageAssignmentVerificationSpec {
        GarageAssignmentVerificationSpec {
            assigned_driver_id: plan.assigned_driver_id.clone(),
            assigned_truck_id: plan.assigned_truck_id.clone(),
            assigned_driver_slot_index: plan.assigned_driver_slot_index,
            assigned_truck_slot_index: plan.assigned_truck_slot_index,
        }
    }

    fn verify_assignment_plan(
        before: &str,
        plan: &GarageResourceAssignmentWritePlan,
        garage_id: &str,
    ) {
        verify_garage_resource_assignment(before, &plan.content, garage_id, &assignment_spec(plan))
            .unwrap();
    }

    #[test]
    fn batch_relinquishment_resets_empty_owned_garages_and_keeps_hq() {
        let targets = vec!["garage.paris".to_string()];
        let plan = apply_garage_relinquishment_batch(SAMPLE, &targets).unwrap();
        let verified = verify_garage_relinquishment_batch(SAMPLE, &plan.content, &targets).unwrap();
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let relinquished = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap();
        let headquarters = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();

        assert_eq!(verified.updated_states.len(), 1);
        assert_eq!(relinquished.ownership, GarageOwnership::NotOwned);
        assert_eq!(relinquished.status, Some(0));
        assert_eq!(relinquished.vehicle_slot_count, 0);
        assert_eq!(relinquished.driver_slot_count, 0);
        assert!(headquarters.is_headquarters);
        assert_eq!(headquarters.slots[0].truck_id.as_deref(), Some("truck.one"));
    }

    #[test]
    fn random_assignment_driver_only_uses_existing_truck_slot() {
        let before = assignment_sample();
        let plan = apply_random_resource_assignment(
            &before,
            "garage.paris",
            assignment_options(true, false),
        )
        .unwrap();
        verify_assignment_plan(&before, &plan, "garage.paris");
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let paris = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap();

        assert!(plan.assigned_driver_id.is_some());
        assert_eq!(plan.assigned_truck_id, None);
        assert_eq!(plan.assigned_driver_slot_index, Some(0));
        assert_eq!(paris.assigned_driver_count, 1);
        assert_eq!(paris.assigned_truck_count, 1);
        assert_eq!(paris.slots[0].truck_id.as_deref(), Some("truck.paris"));
        assert_eq!(paris.slots[0].driver_id, plan.assigned_driver_id);
    }

    #[test]
    fn random_assignment_truck_only_uses_free_vehicle_slot() {
        let before = assignment_sample();
        let plan = apply_random_resource_assignment(
            &before,
            "garage.paris",
            assignment_options(false, true),
        )
        .unwrap();
        verify_assignment_plan(&before, &plan, "garage.paris");
        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let paris = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap();

        assert_eq!(plan.assigned_driver_id, None);
        assert!(plan.assigned_truck_id.is_some());
        assert!(matches!(plan.assigned_truck_slot_index, Some(1 | 2)));
        assert_eq!(paris.assigned_driver_count, 0);
        assert_eq!(paris.assigned_truck_count, 2);
    }

    #[test]
    fn random_assignment_driver_and_truck_can_use_new_truck_slot() {
        let before = assignment_sample_without_paris_truck();
        let plan = apply_random_resource_assignment(
            &before,
            "garage.paris",
            assignment_options(true, true),
        )
        .unwrap();
        verify_assignment_plan(&before, &plan, "garage.paris");

        assert!(plan.assigned_driver_id.is_some());
        assert!(plan.assigned_truck_id.is_some());
        assert_eq!(
            plan.assigned_driver_slot_index,
            plan.assigned_truck_slot_index
        );
    }

    #[test]
    fn random_assignment_rejects_disabled_options_without_write() {
        let error = apply_random_resource_assignment(
            &assignment_sample(),
            "garage.paris",
            assignment_options(false, false),
        )
        .unwrap_err();
        assert_eq!(error, "garage_assignment_empty");
    }

    #[test]
    fn random_assignment_rejects_garage_without_free_slots() {
        let before = assignment_sample_with_full_paris();
        let truck_error = apply_random_resource_assignment(
            &before,
            "garage.paris",
            assignment_options(false, true),
        )
        .unwrap_err();
        let driver_error = apply_random_resource_assignment(
            &before,
            "garage.paris",
            assignment_options(true, false),
        )
        .unwrap_err();

        assert_eq!(truck_error, "garage_assignment_no_free_vehicle_slot");
        assert_eq!(driver_error, "garage_assignment_no_free_driver_slot");
    }

    #[test]
    fn random_assignment_rejects_missing_available_driver_or_truck() {
        let truck_error = apply_random_resource_assignment(
            &assignment_sample_without_available_trucks(),
            "garage.paris",
            assignment_options(false, true),
        )
        .unwrap_err();
        let driver_error = apply_random_resource_assignment(
            &assignment_sample_without_available_drivers(),
            "garage.paris",
            assignment_options(true, false),
        )
        .unwrap_err();

        assert_eq!(truck_error, "garage_assignment_no_available_truck");
        assert_eq!(driver_error, "garage_assignment_no_available_driver");
    }

    #[test]
    fn random_assignment_supports_multiple_garages_sequentially() {
        let before = assignment_sample();
        let first = apply_random_resource_assignment(
            &before,
            "garage.paris",
            assignment_options(false, true),
        )
        .unwrap();
        verify_assignment_plan(&before, &first, "garage.paris");
        let second = apply_random_resource_assignment(
            &first.content,
            "garage.berlin",
            assignment_options(true, false),
        )
        .unwrap();
        verify_assignment_plan(&first.content, &second, "garage.berlin");
        let parsed = parse_garages_from_sii(&second.content).unwrap();
        let paris = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap();
        let berlin = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();

        assert_eq!(paris.assigned_truck_count, 2);
        assert_eq!(berlin.assigned_driver_count, 2);
        assert_ne!(first.content, second.content);
    }

    #[test]
    fn atomic_write_roundtrip_verifies_random_assignment_after_reload() {
        let before = assignment_sample_without_paris_truck();
        let path = std::env::temp_dir().join(format!(
            "ets2-garage-assignment-roundtrip-{}.sii",
            Uuid::new_v4()
        ));
        fs::write(&path, &before).unwrap();
        let plan = apply_random_resource_assignment(
            &before,
            "garage.paris",
            assignment_options(true, true),
        )
        .unwrap();
        let spec = assignment_spec(&plan);

        write_verified_content(&path, &plan.content, |candidate| {
            verify_garage_resource_assignment(&before, candidate, "garage.paris", &spec).map(|_| ())
        })
        .unwrap();

        let written = fs::read_to_string(&path).unwrap();
        let verified =
            verify_garage_resource_assignment(&before, &written, "garage.paris", &spec).unwrap();
        assert_eq!(verified.updated_state.assigned_truck_count, 1);
        assert_eq!(verified.updated_state.assigned_driver_count, 1);
        fs::remove_file(path).unwrap();
    }

    fn save_unit_counts(content: &str) -> (usize, usize, usize, usize) {
        let blocks = parse_unit_blocks(content);
        (
            blocks
                .iter()
                .filter(|block| block.unit_type == "vehicle")
                .count(),
            blocks
                .iter()
                .filter(|block| block.unit_type == "driver_ai")
                .count(),
            blocks
                .iter()
                .filter(|block| block.unit_type == "driver_player")
                .count(),
            blocks
                .iter()
                .filter(|block| block.unit_type == "trailer")
                .count(),
        )
    }
}
