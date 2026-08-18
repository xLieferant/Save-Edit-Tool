use std::collections::{BTreeMap, HashSet};

use rand::seq::SliceRandom;

use crate::features::garages::parser::parse_garages_from_sii;
use crate::features::truck_change::parser::{
    UnitBlock, extract_array_entries, extract_field_value, is_null_ref, normalize_sii_unit_id,
    parse_unit_blocks,
};

use super::parser::{
    ParsedAiDriverPool, driver_ai_block_counts, economy_block, parse_ai_driver_pool,
    parse_ai_driver_pool_from_blocks, player_block,
};

const LINE_FEED: char = 10 as char;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverAssignmentPlan {
    pub content: String,
    pub changed_unit_ids: Vec<String>,
    pub assigned_driver_ids: Vec<String>,
    pub assigned_driver_slot_indices: Vec<usize>,
    pub remaining_free_slots: usize,
    pub remaining_pool_size: usize,
    pub warnings: Vec<String>,
}

pub fn apply_random_ai_driver_assignment(
    content: &str,
    garage_id: &str,
    count: usize,
) -> Result<DriverAssignmentPlan, String> {
    let unit_blocks = parse_unit_blocks(content);
    let context = assignment_context(&unit_blocks, garage_id)?;
    if count == 0 {
        return Err("garage_assignment_driver_count_invalid:zero".to_string());
    }
    if context.free_slots.is_empty() {
        return Err("garage_assignment_no_free_driver_slot".to_string());
    }
    if count > context.free_slots.len() {
        return Err("garage_assignment_driver_count_invalid:free_slots".to_string());
    }

    let mut available_driver_ids = available_pool_driver_ids(
        &unit_blocks,
        &context.parsed_pool,
        &context.assigned_driver_ids,
    );
    if available_driver_ids.is_empty() {
        return Err("garage_assignment_no_available_driver".to_string());
    }
    if count > available_driver_ids.len() {
        return Err("garage_assignment_driver_count_invalid:available_drivers".to_string());
    }
    shuffle_driver_ids(&mut available_driver_ids);

    let assigned_driver_ids = available_driver_ids
        .into_iter()
        .take(count)
        .collect::<Vec<_>>();
    apply_ai_driver_assignment_with_context(
        content,
        &unit_blocks,
        garage_id,
        context,
        assigned_driver_ids,
    )
}

pub fn apply_ai_driver_assignment(
    content: &str,
    garage_id: &str,
    driver_ref: &str,
) -> Result<DriverAssignmentPlan, String> {
    let unit_blocks = parse_unit_blocks(content);
    let context = assignment_context(&unit_blocks, garage_id)?;
    if context.free_slots.is_empty() {
        return Err("garage_assignment_no_free_driver_slot".to_string());
    }

    let driver_id = normalize_sii_unit_id(driver_ref.trim());
    if driver_id.is_empty() || is_null_ref(&driver_id) {
        return Err("garage_assignment_no_available_driver:driver_ref_invalid".to_string());
    }
    validate_requested_driver(
        &unit_blocks,
        &context.parsed_pool,
        &context.assigned_driver_ids,
        &driver_id,
    )?;

    apply_ai_driver_assignment_with_context(
        content,
        &unit_blocks,
        garage_id,
        context,
        vec![driver_id],
    )
}

struct DriverAssignmentContext {
    garage_block: UnitBlock,
    free_slots: Vec<usize>,
    parsed_pool: ParsedAiDriverPool,
    assigned_driver_ids: HashSet<String>,
}

fn assignment_context(
    unit_blocks: &[UnitBlock],
    garage_id: &str,
) -> Result<DriverAssignmentContext, String> {
    let garage_block = unique_unit_block_from_blocks(unit_blocks, "garage", garage_id)?;
    let driver_count = parse_array_count(&garage_block, "drivers")?;
    let drivers = array_map(&garage_block.raw_block, "drivers", driver_count)?;
    let mut free_slots = (0..driver_count)
        .filter(|index| drivers.get(index).is_none_or(|value| is_null_ref(value)))
        .collect::<Vec<_>>();
    free_slots.sort_unstable();
    let parsed_pool = parse_ai_driver_pool_from_blocks(unit_blocks)?;
    let assigned_driver_ids = super::parser::assigned_driver_ids(unit_blocks)
        .into_iter()
        .collect::<HashSet<_>>();

    Ok(DriverAssignmentContext {
        garage_block,
        free_slots,
        parsed_pool,
        assigned_driver_ids,
    })
}

fn available_pool_driver_ids(
    unit_blocks: &[UnitBlock],
    parsed_pool: &ParsedAiDriverPool,
    assigned_driver_ids: &HashSet<String>,
) -> Vec<String> {
    let driver_block_counts = driver_ai_block_counts(unit_blocks);
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for entry in &parsed_pool.drivers {
        if assigned_driver_ids.contains(&entry.driver_id)
            || driver_block_counts.get(&entry.driver_id).copied() != Some(1)
            || !seen.insert(entry.driver_id.clone())
        {
            continue;
        }
        candidates.push(entry.driver_id.clone());
    }
    candidates
}

fn validate_requested_driver(
    unit_blocks: &[UnitBlock],
    parsed_pool: &ParsedAiDriverPool,
    assigned_driver_ids: &HashSet<String>,
    driver_id: &str,
) -> Result<(), String> {
    if !parsed_pool
        .drivers
        .iter()
        .any(|entry| entry.driver_id == driver_id)
    {
        return Err(format!(
            "garage_assignment_no_available_driver:not_in_pool:{driver_id}"
        ));
    }
    if assigned_driver_ids.contains(driver_id) {
        return Err(format!(
            "garage_assignment_no_available_driver:already_assigned:{driver_id}"
        ));
    }
    match driver_ai_block_counts(unit_blocks)
        .get(driver_id)
        .copied()
        .unwrap_or(0)
    {
        1 => Ok(()),
        0 => Err(format!(
            "garage_assignment_no_available_driver:driver_block_missing:{driver_id}"
        )),
        _ => Err(format!(
            "garage_assignment_no_available_driver:driver_block_ambiguous:{driver_id}"
        )),
    }
}

fn apply_ai_driver_assignment_with_context(
    content: &str,
    unit_blocks: &[UnitBlock],
    garage_id: &str,
    context: DriverAssignmentContext,
    assigned_driver_ids: Vec<String>,
) -> Result<DriverAssignmentPlan, String> {
    if assigned_driver_ids.is_empty() {
        return Err("save_verification_failed:assignment_empty".to_string());
    }
    if assigned_driver_ids.len() > context.free_slots.len() {
        return Err("garage_assignment_driver_count_invalid:free_slots".to_string());
    }
    let selected_set = assigned_driver_ids.iter().cloned().collect::<HashSet<_>>();
    if selected_set.len() != assigned_driver_ids.len() {
        return Err("garage_assignment_no_available_driver:duplicate_request".to_string());
    }

    let assigned_driver_slot_indices = context
        .free_slots
        .iter()
        .copied()
        .take(assigned_driver_ids.len())
        .collect::<Vec<_>>();

    let mut garage_raw = context.garage_block.raw_block.clone();
    for (slot_index, driver_id) in assigned_driver_slot_indices
        .iter()
        .copied()
        .zip(assigned_driver_ids.iter())
    {
        garage_raw = replace_array_value(&garage_raw, "drivers", slot_index, driver_id)?;
    }

    let economy_block = economy_block(unit_blocks)?;
    let remaining_pool_driver_ids = context
        .parsed_pool
        .drivers
        .iter()
        .filter(|entry| {
            !selected_set.contains(&entry.driver_id)
                && !context.assigned_driver_ids.contains(&entry.driver_id)
        })
        .map(|entry| entry.driver_id.clone())
        .collect::<Vec<_>>();
    let economy_raw = rewrite_counted_array(
        &economy_block.raw_block,
        "driver_pool",
        &remaining_pool_driver_ids,
    )?;
    let economy_id = economy_block.id.clone();

    let player_block = player_block(unit_blocks)
        .ok_or_else(|| "driver_pool_invalid:player_missing".to_string())?;
    let player_raw = append_player_driver_roster(&player_block.raw_block, &assigned_driver_ids)?;

    let mut replacements = vec![
        (economy_block, economy_raw),
        (player_block, player_raw),
        (context.garage_block, garage_raw),
    ];
    replacements.sort_by(|(left, _), (right, _)| right.start_line.cmp(&left.start_line));
    let mut updated = content.to_string();
    for (block, rewritten_block) in replacements {
        updated = replace_unit_block(&updated, &block, &rewritten_block)?;
    }

    let mut changed_unit_ids = vec![garage_id.to_string(), economy_id];
    changed_unit_ids.extend(assigned_driver_ids.iter().cloned());
    changed_unit_ids.sort();
    changed_unit_ids.dedup();

    Ok(DriverAssignmentPlan {
        content: updated,
        changed_unit_ids,
        assigned_driver_ids,
        assigned_driver_slot_indices,
        remaining_free_slots: context.free_slots.len().saturating_sub(selected_set.len()),
        remaining_pool_size: remaining_pool_driver_ids.len(),
        warnings: context.parsed_pool.diagnostics.warnings,
    })
}
pub fn verify_ai_driver_assignment(
    before_content: &str,
    after_content: &str,
    garage_id: &str,
    plan: &DriverAssignmentPlan,
) -> Result<(), String> {
    if plan.assigned_driver_ids.is_empty() {
        return Err("save_verification_failed:assignment_empty".to_string());
    }
    if plan.assigned_driver_ids.len() != plan.assigned_driver_slot_indices.len() {
        return Err("save_verification_failed:assignment_shape".to_string());
    }

    let before_garages = parse_garages_from_sii(before_content)
        .map_err(|error| format!("save_verification_failed:{error}"))?;
    let after_garages = parse_garages_from_sii(after_content)
        .map_err(|error| format!("save_verification_failed:{error}"))?;
    let before_target = before_garages
        .garages
        .iter()
        .find(|garage| garage.garage_id == garage_id)
        .ok_or_else(|| format!("save_verification_failed:garage_missing:{garage_id}"))?;
    let after_target = after_garages
        .garages
        .iter()
        .find(|garage| garage.garage_id == garage_id)
        .ok_or_else(|| format!("save_verification_failed:garage_missing:{garage_id}"))?;

    if before_garages.garages.len() != after_garages.garages.len()
        || before_garages
            .garages
            .iter()
            .map(|garage| garage.garage_id.as_str())
            .collect::<Vec<_>>()
            != after_garages
                .garages
                .iter()
                .map(|garage| garage.garage_id.as_str())
                .collect::<Vec<_>>()
    {
        return Err("save_verification_failed:garage_order_changed".to_string());
    }

    for (before, after) in before_garages
        .garages
        .iter()
        .zip(after_garages.garages.iter())
        .filter(|(before, _)| before.garage_id != garage_id)
    {
        if before.slots != after.slots || garage_static_fields_changed(before, after) {
            return Err("save_verification_failed:other_garage_changed".to_string());
        }
    }

    if garage_static_fields_changed(before_target, after_target) {
        return Err("save_verification_failed:garage_metadata_changed".to_string());
    }

    let assignment_pairs = plan
        .assigned_driver_slot_indices
        .iter()
        .copied()
        .zip(plan.assigned_driver_ids.iter())
        .collect::<Vec<_>>();
    for slot in &before_target.slots {
        let after_slot = after_target
            .slots
            .iter()
            .find(|candidate| candidate.index == slot.index)
            .ok_or_else(|| "save_verification_failed:slot_missing".to_string())?;
        if let Some((_, driver_id)) = assignment_pairs
            .iter()
            .find(|(slot_index, _)| *slot_index == slot.index)
        {
            if slot.driver_id.is_some() || after_slot.driver_id.as_deref() != Some(driver_id) {
                return Err("save_verification_failed:driver_slot_not_assigned".to_string());
            }
            if slot.truck_id != after_slot.truck_id {
                return Err("save_verification_failed:truck_slot_changed".to_string());
            }
        } else if slot != after_slot {
            return Err("save_verification_failed:untargeted_slot_changed".to_string());
        }
    }

    verify_no_duplicate_garage_drivers(&after_garages.garages)?;
    verify_pool_and_player_roster(before_content, after_content, plan)
}

fn garage_static_fields_changed(
    before: &crate::features::garages::models::GarageInfo,
    after: &crate::features::garages::models::GarageInfo,
) -> bool {
    before.ownership != after.ownership
        || before.size != after.size
        || before.status != after.status
        || before.vehicle_slot_count != after.vehicle_slot_count
        || before.driver_slot_count != after.driver_slot_count
        || before.trailer_slot_count != after.trailer_slot_count
        || before.trailer_ids != after.trailer_ids
        || before.profit_log_id != after.profit_log_id
        || before.productivity != after.productivity
}
fn verify_pool_and_player_roster(
    before_content: &str,
    after_content: &str,
    plan: &DriverAssignmentPlan,
) -> Result<(), String> {
    let before_pool = parse_ai_driver_pool(before_content)
        .map_err(|error| format!("save_verification_failed:{error}"))?;
    let after_pool = parse_ai_driver_pool(after_content)
        .map_err(|error| format!("save_verification_failed:{error}"))?;
    let selected = plan
        .assigned_driver_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let before_pool_set = before_pool
        .drivers
        .iter()
        .map(|entry| entry.driver_id.clone())
        .collect::<HashSet<_>>();
    let after_pool_set = after_pool
        .drivers
        .iter()
        .map(|entry| entry.driver_id.clone())
        .collect::<HashSet<_>>();
    if selected
        .iter()
        .any(|driver_id| !before_pool_set.contains(driver_id) || after_pool_set.contains(driver_id))
    {
        return Err("save_verification_failed:driver_pool_not_updated".to_string());
    }
    let before_blocks = parse_unit_blocks(before_content);
    let before_assigned_set = super::parser::assigned_driver_ids(&before_blocks)
        .into_iter()
        .collect::<HashSet<_>>();
    let expected_after_pool_size = before_pool_set
        .iter()
        .filter(|driver_id| {
            !selected.contains(*driver_id) && !before_assigned_set.contains(*driver_id)
        })
        .count();
    if after_pool.drivers.len() != expected_after_pool_size {
        return Err("save_verification_failed:driver_pool_size".to_string());
    }
    if after_pool.diagnostics.declared_count != after_pool.drivers.len()
        || after_pool
            .diagnostics
            .warnings
            .iter()
            .any(|warning| warning.contains("_indices_invalid"))
    {
        return Err("save_verification_failed:driver_pool_indices".to_string());
    }

    let after_blocks = parse_unit_blocks(after_content);
    let before_player = player_block(&before_blocks)
        .ok_or_else(|| "save_verification_failed:player_missing".to_string())?;
    let after_player = player_block(&after_blocks)
        .ok_or_else(|| "save_verification_failed:player_missing".to_string())?;
    let before_drivers = parse_gapless_array(&before_player.raw_block, "drivers")?;
    let after_drivers = parse_gapless_array(&after_player.raw_block, "drivers")?;
    if after_drivers.len() != before_drivers.len() + selected.len() {
        return Err("save_verification_failed:player_driver_roster_size".to_string());
    }
    let appended = &after_drivers[before_drivers.len()..];
    if appended != plan.assigned_driver_ids.as_slice() {
        return Err("save_verification_failed:player_driver_roster_order".to_string());
    }
    for field in [
        "driver_flags",
        "driver_readiness_timer",
        "driver_undrivable_truck_timers",
    ] {
        let before_values = parse_gapless_array(&before_player.raw_block, field)?;
        let after_values = parse_gapless_array(&after_player.raw_block, field)?;
        if before_values.len() != before_drivers.len()
            || after_values.len() != after_drivers.len()
            || after_values[before_values.len()..]
                .iter()
                .any(|value| value != "0")
        {
            return Err(format!("save_verification_failed:{field}_roster"));
        }
    }

    let after_driver_blocks = driver_ai_block_counts(&after_blocks);
    if selected
        .iter()
        .any(|driver_id| after_driver_blocks.get(driver_id).copied() != Some(1))
    {
        return Err("save_verification_failed:driver_ai_missing".to_string());
    }

    Ok(())
}

fn verify_no_duplicate_garage_drivers(
    garages: &[crate::features::garages::models::GarageInfo],
) -> Result<(), String> {
    let mut seen = HashSet::new();
    for garage in garages {
        for slot in &garage.slots {
            let Some(driver_id) = slot.driver_id.as_deref() else {
                continue;
            };
            let normalized = normalize_sii_unit_id(driver_id);
            if !seen.insert(normalized) {
                return Err("save_verification_failed:duplicate_driver_assignment".to_string());
            }
        }
    }
    Ok(())
}

fn append_player_driver_roster(raw_block: &str, driver_ids: &[String]) -> Result<String, String> {
    let mut updated = append_counted_array_values(raw_block, "drivers", driver_ids)?;
    let zeroes = vec!["0".to_string(); driver_ids.len()];
    updated = append_counted_array_values(&updated, "driver_flags", &zeroes)?;
    updated = append_counted_array_values(&updated, "driver_readiness_timer", &zeroes)?;
    append_counted_array_values(&updated, "driver_undrivable_truck_timers", &zeroes)
}

fn append_counted_array_values(
    raw_block: &str,
    field: &str,
    appended_values: &[String],
) -> Result<String, String> {
    let mut values = parse_gapless_array(raw_block, field)?;
    values.extend(appended_values.iter().cloned());
    rewrite_counted_array(raw_block, field, &values)
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

fn parse_gapless_array(raw_block: &str, field: &str) -> Result<Vec<String>, String> {
    let declared = extract_field_value(raw_block, field)
        .ok_or_else(|| format!("driver_pool_invalid:{field}_missing"))?
        .parse::<usize>()
        .map_err(|_| format!("driver_pool_invalid:{field}_invalid"))?;
    let entries = extract_array_entries(raw_block, field);
    let map = entries.iter().cloned().collect::<BTreeMap<_, _>>();
    if entries.len() != declared
        || map.len() != declared
        || (0..declared).any(|index| !map.contains_key(&index))
    {
        return Err(format!("driver_pool_invalid:{field}_indices_invalid"));
    }
    Ok((0..declared)
        .filter_map(|index| map.get(&index).cloned())
        .collect())
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
        [] => Err(format!("driver_pool_invalid:{unit_type}_missing")),
        [block] => Ok((**block).clone()),
        _ => Err(format!("garage_reference_ambiguous:{unit_id}")),
    }
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

fn rewrite_counted_array(
    raw_block: &str,
    field: &str,
    values: &[String],
) -> Result<String, String> {
    let mut lines = raw_block.lines().map(str::to_string).collect::<Vec<_>>();
    let count_prefix = format!("{field}:");
    let count_indices = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(&count_prefix))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let count_index = match count_indices.as_slice() {
        [index] => *index,
        [] => return Err(format!("driver_pool_invalid:{field}_missing")),
        _ => return Err(format!("driver_pool_invalid:{field}_ambiguous")),
    };
    let entry_indent = lines
        .iter()
        .find(|line| array_index_for_line(line, field).is_some())
        .map(|line| line_indent(line))
        .unwrap_or_else(|| line_indent(&lines[count_index]));
    let count_indent = line_indent(&lines[count_index]);
    lines[count_index] = format!("{count_indent}{field}: {}", values.len());
    lines.retain(|line| array_index_for_line(line, field).is_none());
    let new_count_index = lines
        .iter()
        .position(|line| line.trim_start().starts_with(&count_prefix))
        .ok_or_else(|| format!("driver_pool_invalid:{field}_missing"))?;
    let new_entries = values
        .iter()
        .enumerate()
        .map(|(index, value)| format!("{entry_indent}{field}[{index}]: {value}"))
        .collect::<Vec<_>>();
    lines.splice(new_count_index + 1..new_count_index + 1, new_entries);
    Ok(join_lines(&lines))
}

fn replace_unit_block(
    content: &str,
    block: &UnitBlock,
    rewritten_block: &str,
) -> Result<String, String> {
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    if block.end_line >= lines.len() || block.start_line > block.end_line {
        return Err("driver_pool_invalid:block_range".to_string());
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

fn array_index_for_line(line: &str, field: &str) -> Option<usize> {
    let prefix = format!("{field}[");
    let suffix = line.trim_start().strip_prefix(&prefix)?;
    let (index, _) = suffix.split_once("]:")?;
    index.parse::<usize>().ok()
}

fn line_indent(line: &str) -> String {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .collect()
}

fn join_lines(lines: &[String]) -> String {
    lines.join(&LINE_FEED.to_string())
}

fn shuffle_driver_ids(values: &mut [String]) {
    let mut rng = rand::thread_rng();
    values.shuffle(&mut rng);
}

#[cfg(test)]
mod tests {
    use super::{
        apply_ai_driver_assignment, apply_random_ai_driver_assignment, parse_gapless_array,
        verify_ai_driver_assignment,
    };
    use crate::features::drivers::parser::parse_ai_driver_pool;
    use crate::features::garages::parser::parse_garages_from_sii;
    use crate::features::garages::writer::write_verified_content;
    use crate::features::truck_change::parser::{
        extract_array_entries, extract_field_value, parse_unit_blocks,
    };
    use std::fs;
    use uuid::Uuid;

    const SAMPLE: &str = r#"SiiNunit
{
economy : _economy {
 player: _player
 garages: 2
 garages[0]: garage.berlin
 garages[1]: garage.paris
 driver_pool: 4
 driver_pool[0]: driver.free_a
 driver_pool[1]: driver.free_b
 driver_pool[2]: driver.free_c
 driver_pool[3]: driver.hired_elsewhere
}
player : _player {
 drivers: 1
 drivers[0]: driver.hired_elsewhere
 driver_flags: 1
 driver_flags[0]: 0
 driver_readiness_timer: 1
 driver_readiness_timer[0]: 0
 driver_undrivable_truck_timers: 1
 driver_undrivable_truck_timers[0]: 0
 driver_quit_warned: 0
}
garage : garage.berlin {
 vehicles: 3
 vehicles[0]: null
 vehicles[1]: null
 vehicles[2]: null
 drivers: 3
 drivers[0]: null
 drivers[1]: driver.keep
 drivers[2]: null
 trailers: 0
 status: 2
 profit_log: profit.berlin
 productivity: 0
}
profit_log : profit.berlin {
}
garage : garage.paris {
 vehicles: 3
 vehicles[0]: null
 vehicles[1]: null
 vehicles[2]: null
 drivers: 3
 drivers[0]: driver.hired_elsewhere
 drivers[1]: null
 drivers[2]: null
 trailers: 0
 status: 2
 profit_log: profit.paris
 productivity: 0
}
profit_log : profit.paris {
}
driver_ai : driver.keep {
 assigned_truck: null
}
driver_ai : driver.hired_elsewhere {
 assigned_truck: null
}
driver_ai : driver.free_a {
 assigned_truck: null
}
driver_ai : driver.free_b {
 assigned_truck: null
}
driver_ai : driver.free_c {
 assigned_truck: null
}
}
"#;

    const SAMPLE_DRIVER_POOL: &str = " driver_pool: 4\n driver_pool[0]: driver.free_a\n driver_pool[1]: driver.free_b\n driver_pool[2]: driver.free_c\n driver_pool[3]: driver.hired_elsewhere";

    fn sample_with_driver_pool(driver_ids: &[&str]) -> String {
        let replacement = if driver_ids.is_empty() {
            " driver_pool: 0".to_string()
        } else {
            let entries = driver_ids
                .iter()
                .enumerate()
                .map(|(index, driver_id)| format!(" driver_pool[{index}]: {driver_id}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(" driver_pool: {}\n{entries}", driver_ids.len())
        };
        SAMPLE.replace(SAMPLE_DRIVER_POOL, &replacement)
    }

    fn sample_with_full_berlin_garage() -> String {
        SAMPLE.replace(
            " drivers[0]: null\n drivers[1]: driver.keep\n drivers[2]: null",
            " drivers[0]: driver.free_a\n drivers[1]: driver.keep\n drivers[2]: driver.free_b",
        )
    }

    #[test]
    fn assign_ai_driver_e2e_writes_reload_and_verifies_specific_slot() {
        let path =
            std::env::temp_dir().join(format!("ets2-driver-assignment-e2e-{}.sii", Uuid::new_v4()));
        fs::write(&path, SAMPLE).unwrap();
        let before_content = fs::read_to_string(&path).unwrap();
        let before_garages = parse_garages_from_sii(&before_content).unwrap();
        let before_target = before_garages
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap()
            .clone();
        let before_other = before_garages
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap()
            .clone();

        let plan =
            apply_ai_driver_assignment(&before_content, "garage.berlin", "driver.free_a").unwrap();
        verify_ai_driver_assignment(&before_content, &plan.content, "garage.berlin", &plan)
            .unwrap();
        write_verified_content(&path, &plan.content, |candidate| {
            verify_ai_driver_assignment(&before_content, candidate, "garage.berlin", &plan)
        })
        .unwrap();

        let reloaded = fs::read_to_string(&path).unwrap();
        verify_ai_driver_assignment(&before_content, &reloaded, "garage.berlin", &plan).unwrap();
        let after_garages = parse_garages_from_sii(&reloaded).unwrap();
        let after_target = after_garages
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();
        let after_other = after_garages
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.paris")
            .unwrap();

        assert_eq!(plan.assigned_driver_ids, vec!["driver.free_a".to_string()]);
        assert_eq!(plan.assigned_driver_slot_indices, vec![0]);
        assert_eq!(
            after_target.slots[0].driver_id.as_deref(),
            Some("driver.free_a")
        );
        assert_eq!(
            after_target.slots[1].driver_id.as_deref(),
            Some("driver.keep")
        );
        assert_eq!(after_target.status, before_target.status);
        assert_eq!(after_target.profit_log_id, before_target.profit_log_id);
        assert_eq!(after_target.productivity, before_target.productivity);
        assert_eq!(after_target.trailer_ids, before_target.trailer_ids);
        assert_eq!(
            after_target
                .slots
                .iter()
                .map(|slot| slot.truck_id.clone())
                .collect::<Vec<_>>(),
            before_target
                .slots
                .iter()
                .map(|slot| slot.truck_id.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(after_other.slots, before_other.slots);
        assert_eq!(after_other.trailer_ids, before_other.trailer_ids);
        assert_eq!(after_other.status, before_other.status);
        assert_eq!(after_other.profit_log_id, before_other.profit_log_id);
        assert_eq!(after_other.productivity, before_other.productivity);

        let garage_block = parse_unit_blocks(&reloaded)
            .into_iter()
            .find(|block| block.unit_type == "garage" && block.id == "garage.berlin")
            .unwrap();
        assert_eq!(
            extract_field_value(&garage_block.raw_block, "drivers").as_deref(),
            Some("3")
        );
        assert_eq!(
            extract_array_entries(&garage_block.raw_block, "drivers").len(),
            3
        );
        let after_pool = parse_ai_driver_pool(&reloaded).unwrap();
        assert!(
            !after_pool
                .available_driver_ids
                .iter()
                .any(|driver_id| driver_id == "driver.free_a")
        );
        let assigned = after_garages
            .garages
            .iter()
            .flat_map(|garage| garage.slots.iter())
            .filter_map(|slot| slot.driver_id.as_deref())
            .collect::<Vec<_>>();
        let unique = assigned.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(assigned.len(), unique.len());

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn assigns_specific_driver_and_rejects_unsafe_driver_refs() {
        let plan = apply_ai_driver_assignment(SAMPLE, "garage.berlin", "driver.free_b").unwrap();
        assert_eq!(plan.assigned_driver_ids, vec!["driver.free_b".to_string()]);
        assert_eq!(plan.assigned_driver_slot_indices, vec![0]);

        let assigned =
            apply_ai_driver_assignment(SAMPLE, "garage.berlin", "driver.hired_elsewhere")
                .unwrap_err();
        assert!(assigned.contains("garage_assignment_no_available_driver"));

        let missing =
            apply_ai_driver_assignment(SAMPLE, "garage.berlin", "driver.missing").unwrap_err();
        assert!(missing.contains("garage_assignment_no_available_driver"));
    }

    #[test]
    fn manual_assignment_rejects_missing_garage_empty_pool_and_invalid_save() {
        let missing_garage =
            apply_ai_driver_assignment(SAMPLE, "garage.missing", "driver.free_a").unwrap_err();
        assert!(missing_garage.contains("garage_not_found"));

        let empty_pool = SAMPLE.replace(
            " driver_pool: 4\n driver_pool[0]: driver.free_a\n driver_pool[1]: driver.free_b\n driver_pool[2]: driver.free_c\n driver_pool[3]: driver.hired_elsewhere",
            " driver_pool: 0",
        );
        let empty_pool_error =
            apply_ai_driver_assignment(&empty_pool, "garage.berlin", "driver.free_a").unwrap_err();
        assert!(empty_pool_error.contains("garage_assignment_no_available_driver"));

        let invalid = "SiiNunit\n{\ngarage : garage.berlin {\n drivers: nope\n}\n}";
        let invalid_error =
            apply_ai_driver_assignment(invalid, "garage.berlin", "driver.free_a").unwrap_err();
        assert!(invalid_error.contains("garage_block_invalid"));
    }

    #[test]
    fn manual_assignment_rejects_garage_without_free_driver_slots() {
        let full = SAMPLE
            .replace(" drivers[0]: null", " drivers[0]: driver.free_a")
            .replace(" drivers[2]: null", " drivers[2]: driver.free_b");
        let error =
            apply_ai_driver_assignment(&full, "garage.berlin", "driver.free_c").unwrap_err();
        assert_eq!(error, "garage_assignment_no_free_driver_slot");
    }
    #[test]
    fn random_assignment_regression_repeats_shuffle_write_reload() {
        let path = std::env::temp_dir().join(format!(
            "ets2-driver-random-assignment-regression-{}.sii",
            Uuid::new_v4()
        ));

        for _ in 0..100 {
            fs::write(&path, SAMPLE).unwrap();
            let before_content = fs::read_to_string(&path).unwrap();
            let plan =
                apply_random_ai_driver_assignment(&before_content, "garage.berlin", 2).unwrap();
            assert_eq!(plan.assigned_driver_ids.len(), 2);
            assert_eq!(plan.assigned_driver_slot_indices, vec![0, 2]);
            let assigned_unique = plan
                .assigned_driver_ids
                .iter()
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(assigned_unique.len(), plan.assigned_driver_ids.len());

            verify_ai_driver_assignment(&before_content, &plan.content, "garage.berlin", &plan)
                .unwrap();
            write_verified_content(&path, &plan.content, |candidate| {
                verify_ai_driver_assignment(&before_content, candidate, "garage.berlin", &plan)
            })
            .unwrap();

            let reloaded = fs::read_to_string(&path).unwrap();
            verify_ai_driver_assignment(&before_content, &reloaded, "garage.berlin", &plan)
                .unwrap();
            let parsed = parse_garages_from_sii(&reloaded).unwrap();
            let berlin = parsed
                .garages
                .iter()
                .find(|garage| garage.garage_id == "garage.berlin")
                .unwrap();
            assert_eq!(berlin.slots[1].driver_id.as_deref(), Some("driver.keep"));
            assert!(berlin.slots[0].driver_id.is_some());
            assert!(berlin.slots[2].driver_id.is_some());
            assert_ne!(berlin.slots[0].driver_id, berlin.slots[2].driver_id);
        }

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn random_assignment_handles_available_driver_count_edges() {
        let no_available = sample_with_driver_pool(&[]);
        assert_eq!(
            apply_random_ai_driver_assignment(&no_available, "garage.berlin", 1).unwrap_err(),
            "garage_assignment_no_available_driver"
        );

        let one_available = sample_with_driver_pool(&["driver.free_a"]);
        let one_plan =
            apply_random_ai_driver_assignment(&one_available, "garage.berlin", 1).unwrap();
        assert_eq!(one_plan.assigned_driver_ids, vec!["driver.free_a"]);
        assert_eq!(one_plan.assigned_driver_slot_indices, vec![0]);
        verify_ai_driver_assignment(
            &one_available,
            &one_plan.content,
            "garage.berlin",
            &one_plan,
        )
        .unwrap();

        let too_many_available =
            apply_random_ai_driver_assignment(&one_available, "garage.berlin", 2).unwrap_err();
        assert_eq!(
            too_many_available,
            "garage_assignment_driver_count_invalid:available_drivers"
        );

        let duplicated_pool =
            sample_with_driver_pool(&["driver.free_a", "driver.free_a", "driver.free_b"]);
        let duplicate_plan =
            apply_random_ai_driver_assignment(&duplicated_pool, "garage.berlin", 2).unwrap();
        let selected = duplicate_plan
            .assigned_driver_ids
            .iter()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(selected.len(), duplicate_plan.assigned_driver_ids.len());
        assert_eq!(selected.len(), 2);
        verify_ai_driver_assignment(
            &duplicated_pool,
            &duplicate_plan.content,
            "garage.berlin",
            &duplicate_plan,
        )
        .unwrap();
    }

    #[test]
    fn random_assignment_rejects_requested_count_over_slots_and_full_garage() {
        let too_many_slots =
            apply_random_ai_driver_assignment(SAMPLE, "garage.berlin", 3).unwrap_err();
        assert_eq!(
            too_many_slots,
            "garage_assignment_driver_count_invalid:free_slots"
        );

        let full = sample_with_full_berlin_garage();
        let full_error = apply_random_ai_driver_assignment(&full, "garage.berlin", 1).unwrap_err();
        assert_eq!(full_error, "garage_assignment_no_free_driver_slot");
    }
    #[test]
    fn assigns_only_free_driver_slots_and_preserves_existing_driver() {
        let plan = apply_random_ai_driver_assignment(SAMPLE, "garage.berlin", 2).unwrap();
        assert_eq!(plan.assigned_driver_ids.len(), 2);
        assert_eq!(plan.assigned_driver_slot_indices, vec![0, 2]);
        verify_ai_driver_assignment(SAMPLE, &plan.content, "garage.berlin", &plan).unwrap();

        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let berlin = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();
        assert_eq!(berlin.slots[1].driver_id.as_deref(), Some("driver.keep"));
        for assigned in &plan.assigned_driver_ids {
            assert!(
                berlin
                    .slots
                    .iter()
                    .any(|slot| slot.driver_id.as_ref() == Some(assigned))
            );
        }
    }

    #[test]
    fn assigns_requested_count_without_filling_all_free_slots() {
        let plan = apply_random_ai_driver_assignment(SAMPLE, "garage.berlin", 1).unwrap();
        assert_eq!(plan.assigned_driver_ids.len(), 1);
        assert_eq!(plan.assigned_driver_slot_indices, vec![0]);
        assert_eq!(plan.remaining_free_slots, 1);
        verify_ai_driver_assignment(SAMPLE, &plan.content, "garage.berlin", &plan).unwrap();

        let parsed = parse_garages_from_sii(&plan.content).unwrap();
        let berlin = parsed
            .garages
            .iter()
            .find(|garage| garage.garage_id == "garage.berlin")
            .unwrap();
        assert_eq!(berlin.slots[1].driver_id.as_deref(), Some("driver.keep"));
        assert!(berlin.slots[2].driver_id.is_none());
    }
    #[test]
    fn removes_assigned_drivers_from_pool_gaplessly() {
        let plan = apply_random_ai_driver_assignment(SAMPLE, "garage.berlin", 2).unwrap();
        let after_pool = parse_ai_driver_pool(&plan.content).unwrap();
        assert_eq!(
            after_pool.diagnostics.declared_count,
            after_pool.drivers.len()
        );
        for (expected_index, entry) in after_pool.drivers.iter().enumerate() {
            assert_eq!(entry.index, expected_index);
            assert!(!plan.assigned_driver_ids.contains(&entry.driver_id));
        }
        assert!(
            !after_pool
                .drivers
                .iter()
                .any(|entry| entry.driver_id == "driver.hired_elsewhere")
        );
    }

    #[test]
    fn appends_player_driver_roster_and_timer_arrays() {
        let plan = apply_random_ai_driver_assignment(SAMPLE, "garage.berlin", 2).unwrap();
        let blocks = parse_unit_blocks(&plan.content);
        let player = blocks
            .iter()
            .find(|block| block.unit_type == "player")
            .unwrap();
        let drivers = parse_gapless_array(&player.raw_block, "drivers").unwrap();
        assert_eq!(drivers.len(), 3);
        assert_eq!(&drivers[1..], plan.assigned_driver_ids.as_slice());
        for field in [
            "driver_flags",
            "driver_readiness_timer",
            "driver_undrivable_truck_timers",
        ] {
            let values = parse_gapless_array(&player.raw_block, field).unwrap();
            assert_eq!(values.len(), drivers.len());
            assert_eq!(&values[1..], &["0".to_string(), "0".to_string()]);
        }
    }

    #[test]
    fn skips_already_assigned_driver_even_if_pool_contains_it() {
        let plan = apply_random_ai_driver_assignment(SAMPLE, "garage.berlin", 2).unwrap();
        assert!(
            !plan
                .assigned_driver_ids
                .iter()
                .any(|driver_id| driver_id == "driver.hired_elsewhere")
        );
    }

    #[test]
    fn rejects_driver_count_outside_safe_limits() {
        let zero = apply_random_ai_driver_assignment(SAMPLE, "garage.berlin", 0).unwrap_err();
        assert!(zero.contains("garage_assignment_driver_count_invalid"));

        let too_many = apply_random_ai_driver_assignment(SAMPLE, "garage.berlin", 3).unwrap_err();
        assert!(too_many.contains("garage_assignment_driver_count_invalid"));
    }
    #[test]
    fn parses_crlf_driver_pool_and_rewrites_indices() {
        let sample = SAMPLE.replace('\n', "\r\n");
        let plan = apply_random_ai_driver_assignment(&sample, "garage.berlin", 2).unwrap();
        verify_ai_driver_assignment(&sample, &plan.content, "garage.berlin", &plan).unwrap();
        let economy = parse_unit_blocks(&plan.content)
            .into_iter()
            .find(|block| block.unit_type == "economy")
            .unwrap();
        let entries = extract_array_entries(&economy.raw_block, "driver_pool");
        for (expected, (index, _)) in entries.iter().enumerate() {
            assert_eq!(*index, expected);
        }
    }
}
