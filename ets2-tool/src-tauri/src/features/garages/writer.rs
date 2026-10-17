use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::features::ets2save::sii_codec::replace_file_atomic;
use crate::features::truck_change::parser::{
    UnitBlock, extract_array_entries, extract_field_value, is_null_ref, normalize_sii_unit_id,
    parse_unit_blocks,
};

use super::parser::{city_token_from_garage_id, parse_ai_driver_management_from_sii};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiDriverWritePlan {
    pub content: String,
    pub changed_unit_ids: Vec<String>,
    pub affected_driver_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct GarageAssignmentState {
    block: UnitBlock,
    vehicles: Vec<String>,
    drivers: Vec<String>,
    owned: bool,
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

    if options.assign_random_driver {
        let plan = apply_ai_driver_generation(content, garage_id, 1, options.assign_random_truck)?;
        let management = parse_ai_driver_management_from_sii(&plan.content, "ets2", "")?;
        let driver_id = plan
            .affected_driver_ids
            .first()
            .cloned()
            .ok_or_else(|| "garage_assignment_no_available_driver".to_string())?;
        let driver = management
            .drivers
            .iter()
            .find(|driver| driver.driver_id.eq_ignore_ascii_case(&driver_id))
            .ok_or_else(|| "garage_assignment_no_available_driver".to_string())?;
        return Ok(GarageResourceAssignmentWritePlan {
            content: plan.content,
            changed_unit_ids: plan.changed_unit_ids,
            assigned_driver_id: Some(driver_id),
            assigned_truck_id: driver.truck_id.clone(),
            assigned_driver_slot_index: Some(driver.garage_slot_index),
            assigned_truck_slot_index: driver.truck_id.as_ref().map(|_| driver.garage_slot_index),
        });
    }

    let (plan, truck_id, slot_index) = move_random_available_truck(content, garage_id)?;
    Ok(GarageResourceAssignmentWritePlan {
        content: plan.content,
        changed_unit_ids: plan.changed_unit_ids,
        assigned_driver_id: None,
        assigned_truck_id: Some(truck_id),
        assigned_driver_slot_index: None,
        assigned_truck_slot_index: Some(slot_index),
    })
}

pub fn apply_ai_driver_generation(
    content: &str,
    garage_id: &str,
    count: usize,
    assign_trucks: bool,
) -> Result<AiDriverWritePlan, String> {
    if count == 0 {
        return Err("ai_driver_count_invalid".to_string());
    }
    let blocks = parse_unit_blocks(content);
    let economy = economy_block_from_blocks(&blocks)?;
    let player = player_block_from_blocks(&blocks)?
        .ok_or_else(|| "garage_block_invalid:player_missing".to_string())?;
    let mut garages = garage_assignment_states(&blocks)?;
    let active_player_trucks = active_player_truck_ids(&player);
    let target = garages
        .get(garage_id)
        .ok_or_else(|| format!("garage_not_found:{garage_id}"))?;
    if !target.owned {
        return Err("ai_driver_garage_not_owned".to_string());
    }

    let target_slots = target
        .drivers
        .iter()
        .enumerate()
        .filter(|(_, driver)| is_null_ref(driver))
        .filter(|(index, _)| {
            let truck = &target.vehicles[*index];
            if assign_trucks {
                is_null_ref(truck) || !active_player_trucks.contains(&normalize_sii_unit_id(truck))
            } else {
                is_null_ref(truck)
            }
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if target_slots.len() < count {
        return Err(format!(
            "ai_driver_insufficient_garage_capacity:available={}:requested={count}",
            target_slots.len()
        ));
    }

    let mut hired = read_array_values(&player, "drivers", "ai_driver_player_array_invalid")?;
    let mut flags = read_array_values(&player, "driver_flags", "ai_driver_player_array_invalid")?;
    let mut readiness = read_array_values(
        &player,
        "driver_readiness_timer",
        "ai_driver_player_array_invalid",
    )?;
    let mut undrivable = read_array_values(
        &player,
        "driver_undrivable_truck_timers",
        "ai_driver_player_array_invalid",
    )?;
    validate_parallel_driver_arrays(&hired, &flags, &readiness, &undrivable)?;
    let mut offers =
        read_array_values(&economy, "drivers_offer", "ai_driver_economy_array_invalid")?;
    let mut pool = read_array_values(&economy, "driver_pool", "ai_driver_economy_array_invalid")?;
    let hired_set = hired
        .iter()
        .map(|value| normalize_sii_unit_id(value))
        .collect::<HashSet<_>>();
    let mut candidates = offers
        .iter()
        .chain(pool.iter())
        .filter(|value| !hired_set.contains(&normalize_sii_unit_id(value)))
        .filter(|value| {
            optional_unique_unit_block(&blocks, "driver_ai", value)
                .ok()
                .flatten()
                .is_some()
        })
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    if candidates.len() < count {
        return Err(format!(
            "ai_driver_insufficient_candidates:available={}:requested={count}",
            candidates.len()
        ));
    }
    let mut selected = Vec::with_capacity(count);
    for _ in 0..count {
        let index = choose_random_usize(&(0..candidates.len()).collect::<Vec<_>>());
        selected.push(candidates.remove(index));
    }

    let available_truck_locations = if assign_trucks {
        free_truck_locations(&garages, &active_player_trucks)
    } else {
        Vec::new()
    };
    if assign_trucks && available_truck_locations.len() < count {
        return Err(format!(
            "ai_driver_insufficient_trucks:available={}:requested={count}",
            available_truck_locations.len()
        ));
    }

    let mut changed_garages = HashSet::new();
    let mut consumed_trucks = HashSet::new();
    for (offset, driver_id) in selected.iter().enumerate() {
        let slot_index = target_slots[offset];
        if assign_trucks {
            let existing_truck = garages
                .get(garage_id)
                .and_then(|garage| garage.vehicles.get(slot_index))
                .cloned()
                .ok_or_else(|| "garage_assignment_no_free_vehicle_slot".to_string())?;
            if is_null_ref(&existing_truck) {
                let (source_garage_id, source_index, truck_id) = available_truck_locations
                    .iter()
                    .find(|(source_garage_id, source_index, truck_id)| {
                        !consumed_trucks.contains(&normalize_sii_unit_id(truck_id))
                            && !(source_garage_id == garage_id && *source_index == slot_index)
                    })
                    .cloned()
                    .ok_or_else(|| "garage_assignment_no_available_truck".to_string())?;
                garages
                    .get_mut(&source_garage_id)
                    .ok_or_else(|| "garage_assignment_no_available_truck".to_string())?
                    .vehicles[source_index] = "null".to_string();
                garages
                    .get_mut(garage_id)
                    .ok_or_else(|| format!("garage_not_found:{garage_id}"))?
                    .vehicles[slot_index] = truck_id.clone();
                consumed_trucks.insert(normalize_sii_unit_id(&truck_id));
                changed_garages.insert(source_garage_id);
            } else {
                consumed_trucks.insert(normalize_sii_unit_id(&existing_truck));
            }
        }
        garages
            .get_mut(garage_id)
            .ok_or_else(|| format!("garage_not_found:{garage_id}"))?
            .drivers[slot_index] = driver_id.clone();
        changed_garages.insert(garage_id.to_string());
        offers.retain(|value| !value.eq_ignore_ascii_case(driver_id));
        pool.retain(|value| !value.eq_ignore_ascii_case(driver_id));
        hired.push(driver_id.clone());
        flags.push("0".to_string());
        readiness.push("0".to_string());
        undrivable.push("0".to_string());
    }

    let player_raw =
        rewrite_player_driver_arrays(&player.raw_block, &hired, &flags, &readiness, &undrivable)?;
    let economy_raw = rewrite_array_field(&economy.raw_block, "drivers_offer", &offers)?;
    let economy_raw = rewrite_array_field(&economy_raw, "driver_pool", &pool)?;
    build_ai_driver_plan(
        content,
        &player,
        player_raw,
        Some((economy, economy_raw)),
        &garages,
        &changed_garages,
        selected,
    )
}

pub fn apply_ai_driver_truck_update(
    content: &str,
    driver_id: &str,
    truck_id: Option<&str>,
) -> Result<AiDriverWritePlan, String> {
    let blocks = parse_unit_blocks(content);
    let player = player_block_from_blocks(&blocks)?
        .ok_or_else(|| "garage_block_invalid:player_missing".to_string())?;
    let hired = read_array_values(&player, "drivers", "ai_driver_player_array_invalid")?;
    if !hired
        .iter()
        .any(|value| value.eq_ignore_ascii_case(driver_id))
    {
        return Err(format!("ai_driver_not_hired:{driver_id}"));
    }
    let mut garages = garage_assignment_states(&blocks)?;
    let (driver_garage_id, driver_slot) = unique_driver_location(&garages, driver_id)?;
    let current_truck = garages[&driver_garage_id].vehicles[driver_slot].clone();
    let active_player_trucks = active_player_truck_ids(&player);
    let mut changed_garages = HashSet::from([driver_garage_id.clone()]);

    if let Some(truck_id) = truck_id.filter(|value| !value.trim().is_empty()) {
        if !is_null_ref(&current_truck) && current_truck.eq_ignore_ascii_case(truck_id) {
            return Err("ai_driver_truck_already_assigned".to_string());
        }
        if active_player_trucks.contains(&normalize_sii_unit_id(truck_id)) {
            return Err("ai_driver_truck_not_available".to_string());
        }
        let (source_garage_id, source_slot) = unique_truck_location(&garages, truck_id)?;
        if !is_null_ref(&garages[&source_garage_id].drivers[source_slot]) {
            return Err("ai_driver_truck_not_available".to_string());
        }
        garages
            .get_mut(&source_garage_id)
            .ok_or_else(|| "ai_driver_truck_not_available".to_string())?
            .vehicles[source_slot] = current_truck;
        garages
            .get_mut(&driver_garage_id)
            .ok_or_else(|| format!("garage_not_found:{driver_garage_id}"))?
            .vehicles[driver_slot] = truck_id.to_string();
        changed_garages.insert(source_garage_id);
    } else {
        if is_null_ref(&current_truck) {
            return Err("ai_driver_truck_already_unassigned".to_string());
        }
        let storage = garages
            .iter()
            .filter(|(_, garage)| garage.owned)
            .flat_map(|(garage_id, garage)| {
                garage
                    .vehicles
                    .iter()
                    .enumerate()
                    .filter(|(index, vehicle)| {
                        is_null_ref(vehicle)
                            && is_null_ref(&garage.drivers[*index])
                            && !(garage_id == &driver_garage_id && *index == driver_slot)
                    })
                    .map(|(index, _)| (garage_id.clone(), index))
                    .collect::<Vec<_>>()
            })
            .next()
            .ok_or_else(|| "ai_driver_no_truck_storage_slot".to_string())?;
        garages
            .get_mut(&storage.0)
            .ok_or_else(|| "ai_driver_no_truck_storage_slot".to_string())?
            .vehicles[storage.1] = current_truck;
        garages
            .get_mut(&driver_garage_id)
            .ok_or_else(|| format!("garage_not_found:{driver_garage_id}"))?
            .vehicles[driver_slot] = "null".to_string();
        changed_garages.insert(storage.0);
    }

    build_ai_driver_plan(
        content,
        &player,
        player.raw_block.clone(),
        None,
        &garages,
        &changed_garages,
        vec![driver_id.to_string()],
    )
}

pub fn apply_ai_driver_removal(
    content: &str,
    driver_ids: &[String],
) -> Result<AiDriverWritePlan, String> {
    if driver_ids.is_empty() {
        return Err("ai_driver_selection_empty".to_string());
    }
    let requested = driver_ids
        .iter()
        .map(|value| normalize_sii_unit_id(value))
        .collect::<HashSet<_>>();
    if requested.len() != driver_ids.len() || requested.contains("") {
        return Err("ai_driver_selection_invalid".to_string());
    }
    let management = parse_ai_driver_management_from_sii(content, "ets2", "")?;
    for driver_id in driver_ids {
        let driver = management
            .drivers
            .iter()
            .find(|driver| driver.driver_id.eq_ignore_ascii_case(driver_id))
            .ok_or_else(|| format!("ai_driver_not_hired:{driver_id}"))?;
        if !driver.removable {
            return Err(format!("ai_driver_active_job:{driver_id}"));
        }
    }

    let blocks = parse_unit_blocks(content);
    let economy = economy_block_from_blocks(&blocks)?;
    let player = player_block_from_blocks(&blocks)?
        .ok_or_else(|| "garage_block_invalid:player_missing".to_string())?;
    let hired = read_array_values(&player, "drivers", "ai_driver_player_array_invalid")?;
    let flags = read_array_values(&player, "driver_flags", "ai_driver_player_array_invalid")?;
    let readiness = read_array_values(
        &player,
        "driver_readiness_timer",
        "ai_driver_player_array_invalid",
    )?;
    let undrivable = read_array_values(
        &player,
        "driver_undrivable_truck_timers",
        "ai_driver_player_array_invalid",
    )?;
    validate_parallel_driver_arrays(&hired, &flags, &readiness, &undrivable)?;
    let mut retained_hired = Vec::new();
    let mut retained_flags = Vec::new();
    let mut retained_readiness = Vec::new();
    let mut retained_undrivable = Vec::new();
    for index in 0..hired.len() {
        if requested.contains(&normalize_sii_unit_id(&hired[index])) {
            continue;
        }
        retained_hired.push(hired[index].clone());
        retained_flags.push(flags[index].clone());
        retained_readiness.push(readiness[index].clone());
        retained_undrivable.push(undrivable[index].clone());
    }
    let offers = read_array_values(&economy, "drivers_offer", "ai_driver_economy_array_invalid")?;
    let mut pool = read_array_values(&economy, "driver_pool", "ai_driver_economy_array_invalid")?;
    for driver_id in driver_ids {
        if !offers
            .iter()
            .any(|value| value.eq_ignore_ascii_case(driver_id))
            && !pool
                .iter()
                .any(|value| value.eq_ignore_ascii_case(driver_id))
        {
            pool.push(driver_id.clone());
        }
    }
    let mut player_raw = rewrite_player_driver_arrays(
        &player.raw_block,
        &retained_hired,
        &retained_flags,
        &retained_readiness,
        &retained_undrivable,
    )?;
    let economy_raw = rewrite_array_field(&economy.raw_block, "drivers_offer", &offers)?;
    let economy_raw = rewrite_array_field(&economy_raw, "driver_pool", &pool)?;
    if let Some(current) = extract_field_value(&player_raw, "dismissed_drivers") {
        let count = current
            .parse::<usize>()
            .map_err(|_| "ai_driver_dismissed_count_invalid".to_string())?;
        player_raw = replace_scalar_field(
            &player_raw,
            "dismissed_drivers",
            &count.saturating_add(driver_ids.len()).to_string(),
        )?;
    }

    let mut garages = garage_assignment_states(&blocks)?;
    let mut changed_garages = HashSet::new();
    for driver_id in driver_ids {
        let (garage_id, slot_index) = unique_driver_location(&garages, driver_id)?;
        garages
            .get_mut(&garage_id)
            .ok_or_else(|| format!("garage_not_found:{garage_id}"))?
            .drivers[slot_index] = "null".to_string();
        changed_garages.insert(garage_id);
    }
    build_ai_driver_plan(
        content,
        &player,
        player_raw,
        Some((economy, economy_raw)),
        &garages,
        &changed_garages,
        driver_ids.to_vec(),
    )
}

fn move_random_available_truck(
    content: &str,
    garage_id: &str,
) -> Result<(AiDriverWritePlan, String, usize), String> {
    let blocks = parse_unit_blocks(content);
    let player = player_block_from_blocks(&blocks)?
        .ok_or_else(|| "garage_block_invalid:player_missing".to_string())?;
    let mut garages = garage_assignment_states(&blocks)?;
    let active_player_trucks = active_player_truck_ids(&player);
    let target = garages
        .get(garage_id)
        .ok_or_else(|| format!("garage_not_found:{garage_id}"))?;
    if !target.owned {
        return Err("ai_driver_garage_not_owned".to_string());
    }
    let target_slots = target
        .vehicles
        .iter()
        .enumerate()
        .filter(|(index, vehicle)| is_null_ref(vehicle) && is_null_ref(&target.drivers[*index]))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if target_slots.is_empty() {
        return Err("garage_assignment_no_free_vehicle_slot".to_string());
    }
    let sources = free_truck_locations(&garages, &active_player_trucks)
        .into_iter()
        .filter(|(source_garage_id, _, _)| source_garage_id != garage_id)
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return Err("garage_assignment_no_available_truck".to_string());
    }
    let target_slot = choose_random_usize(&target_slots);
    let source_index = choose_random_usize(&(0..sources.len()).collect::<Vec<_>>());
    let (source_garage_id, source_slot, truck_id) = sources[source_index].clone();
    garages
        .get_mut(&source_garage_id)
        .ok_or_else(|| "garage_assignment_no_available_truck".to_string())?
        .vehicles[source_slot] = "null".to_string();
    garages
        .get_mut(garage_id)
        .ok_or_else(|| format!("garage_not_found:{garage_id}"))?
        .vehicles[target_slot] = truck_id.clone();
    let changed_garages = HashSet::from([source_garage_id, garage_id.to_string()]);
    let plan = build_ai_driver_plan(
        content,
        &player,
        player.raw_block.clone(),
        None,
        &garages,
        &changed_garages,
        Vec::new(),
    )?;
    Ok((plan, truck_id, target_slot))
}

fn garage_assignment_states(
    blocks: &[UnitBlock],
) -> Result<HashMap<String, GarageAssignmentState>, String> {
    let mut states = HashMap::new();
    for block in blocks.iter().filter(|block| block.unit_type == "garage") {
        if states.contains_key(&block.id) {
            return Err(format!("garage_reference_ambiguous:{}", block.id));
        }
        let vehicles = read_array_values(block, "vehicles", "garage_block_invalid")?;
        let drivers = read_array_values(block, "drivers", "garage_block_invalid")?;
        if vehicles.len() != drivers.len() {
            return Err(format!("garage_capacity_mismatch:{}", block.id));
        }
        let status = extract_field_value(&block.raw_block, "status")
            .and_then(|value| value.parse::<i32>().ok())
            .ok_or_else(|| format!("garage_block_invalid:{}:status_invalid", block.id))?;
        states.insert(
            block.id.clone(),
            GarageAssignmentState {
                block: block.clone(),
                vehicles,
                drivers,
                owned: matches!(status, 2 | 3),
            },
        );
    }
    Ok(states)
}

fn active_player_truck_ids(player: &UnitBlock) -> HashSet<String> {
    ["assigned_truck", "my_truck"]
        .into_iter()
        .filter_map(|field| extract_field_value(&player.raw_block, field))
        .filter(|value| !is_null_ref(value))
        .map(|value| normalize_sii_unit_id(&value))
        .filter(|value| !value.is_empty())
        .collect()
}

fn free_truck_locations(
    garages: &HashMap<String, GarageAssignmentState>,
    active_player_trucks: &HashSet<String>,
) -> Vec<(String, usize, String)> {
    let mut locations = garages
        .iter()
        .filter(|(_, garage)| garage.owned)
        .flat_map(|(garage_id, garage)| {
            garage
                .vehicles
                .iter()
                .enumerate()
                .filter(|(index, truck_id)| {
                    !is_null_ref(truck_id)
                        && is_null_ref(&garage.drivers[*index])
                        && !active_player_trucks.contains(&normalize_sii_unit_id(truck_id))
                })
                .map(|(index, truck_id)| (garage_id.clone(), index, truck_id.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    locations.sort_by(|left, right| left.2.cmp(&right.2));
    locations
}

fn unique_driver_location(
    garages: &HashMap<String, GarageAssignmentState>,
    driver_id: &str,
) -> Result<(String, usize), String> {
    let normalized = normalize_sii_unit_id(driver_id);
    let locations = garages
        .iter()
        .flat_map(|(garage_id, garage)| {
            garage
                .drivers
                .iter()
                .enumerate()
                .filter(|(_, value)| normalize_sii_unit_id(value) == normalized)
                .map(|(index, _)| (garage_id.clone(), index))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    match locations.as_slice() {
        [location] => Ok(location.clone()),
        [] => Err(format!("ai_driver_garage_missing:{driver_id}")),
        _ => Err(format!("ai_driver_garage_ambiguous:{driver_id}")),
    }
}

fn unique_truck_location(
    garages: &HashMap<String, GarageAssignmentState>,
    truck_id: &str,
) -> Result<(String, usize), String> {
    let normalized = normalize_sii_unit_id(truck_id);
    let locations = garages
        .iter()
        .flat_map(|(garage_id, garage)| {
            garage
                .vehicles
                .iter()
                .enumerate()
                .filter(|(_, value)| normalize_sii_unit_id(value) == normalized)
                .map(|(index, _)| (garage_id.clone(), index))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    match locations.as_slice() {
        [location] => Ok(location.clone()),
        [] => Err(format!("ai_driver_truck_not_found:{truck_id}")),
        _ => Err(format!("ai_driver_truck_ambiguous:{truck_id}")),
    }
}

fn read_array_values(
    block: &UnitBlock,
    field: &str,
    error_prefix: &str,
) -> Result<Vec<String>, String> {
    let count = extract_field_value(&block.raw_block, field)
        .ok_or_else(|| format!("{error_prefix}:{field}_missing"))?
        .parse::<usize>()
        .map_err(|_| format!("{error_prefix}:{field}_invalid"))?;
    let entries = extract_array_entries(&block.raw_block, field);
    let values = entries.iter().cloned().collect::<BTreeMap<_, _>>();
    if entries.len() != count
        || values.len() != count
        || (0..count).any(|index| !values.contains_key(&index))
    {
        return Err(format!("{error_prefix}:{field}_indices_invalid"));
    }
    Ok((0..count)
        .filter_map(|index| values.get(&index).cloned())
        .collect())
}

fn validate_parallel_driver_arrays(
    drivers: &[String],
    flags: &[String],
    readiness: &[String],
    undrivable: &[String],
) -> Result<(), String> {
    if flags.len() != drivers.len()
        || readiness.len() != drivers.len()
        || undrivable.len() != drivers.len()
    {
        return Err("ai_driver_player_array_mismatch".to_string());
    }
    Ok(())
}

fn rewrite_player_driver_arrays(
    raw_block: &str,
    drivers: &[String],
    flags: &[String],
    readiness: &[String],
    undrivable: &[String],
) -> Result<String, String> {
    let updated = rewrite_array_field(raw_block, "drivers", drivers)?;
    let updated = rewrite_array_field(&updated, "driver_flags", flags)?;
    let updated = rewrite_array_field(&updated, "driver_readiness_timer", readiness)?;
    rewrite_array_field(&updated, "driver_undrivable_truck_timers", undrivable)
}

fn rewrite_array_field(raw_block: &str, field: &str, values: &[String]) -> Result<String, String> {
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
        [] => return Err(format!("garage_block_invalid:{field}_missing")),
        _ => return Err(format!("garage_block_invalid:{field}_ambiguous")),
    };
    let array_indices = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| array_index_for_line(line, field).is_some())
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let indent = array_indices
        .first()
        .map(|index| line_indent(&lines[*index]))
        .unwrap_or_else(|| line_indent(&lines[count_index]));
    let insert_at = array_indices.first().copied().unwrap_or(count_index + 1);
    lines.retain(|line| array_index_for_line(line, field).is_none());
    let adjusted_insert_at = insert_at.min(lines.len());
    lines[count_index] = format!(
        "{}{field}: {}",
        line_indent(&lines[count_index]),
        values.len()
    );
    lines.splice(
        adjusted_insert_at..adjusted_insert_at,
        values
            .iter()
            .enumerate()
            .map(|(index, value)| format!("{indent}{field}[{index}]: {value}")),
    );
    Ok(join_lines(&lines))
}

fn build_ai_driver_plan(
    content: &str,
    player: &UnitBlock,
    player_raw: String,
    economy_replacement: Option<(UnitBlock, String)>,
    garages: &HashMap<String, GarageAssignmentState>,
    changed_garages: &HashSet<String>,
    affected_driver_ids: Vec<String>,
) -> Result<AiDriverWritePlan, String> {
    let mut replacements = Vec::new();
    if player.raw_block != player_raw {
        replacements.push((player.clone(), player_raw));
    }
    let mut economy_changed = None;
    if let Some((economy, economy_raw)) = economy_replacement {
        if economy.raw_block != economy_raw {
            economy_changed = Some(economy.id.clone());
            replacements.push((economy, economy_raw));
        }
    }
    for garage_id in changed_garages {
        let garage = garages
            .get(garage_id)
            .ok_or_else(|| format!("garage_not_found:{garage_id}"))?;
        let raw = rewrite_array_field(&garage.block.raw_block, "vehicles", &garage.vehicles)?;
        let raw = rewrite_array_field(&raw, "drivers", &garage.drivers)?;
        replacements.push((garage.block.clone(), raw));
    }
    replacements.sort_by(|(left, _), (right, _)| right.start_line.cmp(&left.start_line));
    let mut updated = content.to_string();
    for (block, rewritten) in replacements {
        updated = replace_unit_block(&updated, &block, &rewritten)?;
    }
    let mut changed_unit_ids = changed_garages.iter().cloned().collect::<Vec<_>>();
    if player.raw_block != updated_player_block(content, &updated, &player.id)? {
        changed_unit_ids.push(player.id.clone());
    }
    if let Some(economy_id) = economy_changed {
        changed_unit_ids.push(economy_id);
    }
    changed_unit_ids.sort();
    changed_unit_ids.dedup();
    Ok(AiDriverWritePlan {
        content: updated,
        changed_unit_ids,
        affected_driver_ids,
    })
}

fn updated_player_block(
    before_content: &str,
    after_content: &str,
    player_id: &str,
) -> Result<String, String> {
    let before = unique_unit_block(before_content, "player", player_id)?;
    let after = unique_unit_block(after_content, "player", player_id)?;
    if before.raw_block == after.raw_block {
        Ok(before.raw_block)
    } else {
        Ok(after.raw_block)
    }
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
    let economy_block = economy_block_from_blocks(unit_blocks)?;
    let player_id = extract_field_value(&economy_block.raw_block, "player")
        .ok_or_else(|| "garage_block_invalid:player_reference_missing".to_string())?;
    optional_unique_unit_block(unit_blocks, "player", &player_id)
}

fn economy_block_from_blocks(unit_blocks: &[UnitBlock]) -> Result<UnitBlock, String> {
    let economy_blocks = unit_blocks
        .iter()
        .filter(|block| block.unit_type == "economy")
        .collect::<Vec<_>>();
    match economy_blocks.as_slice() {
        [block] => Ok((**block).clone()),
        [] => Err("garage_block_invalid:economy_missing".to_string()),
        _ => Err("garage_reference_ambiguous:economy".to_string()),
    }
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
        GarageResourceAssignmentOptions, GarageResourceAssignmentWritePlan,
        apply_ai_driver_generation, apply_ai_driver_removal, apply_ai_driver_truck_update,
        apply_garage_changes, apply_garage_purchase_batch, apply_garage_relinquishment,
        apply_garage_relinquishment_batch, apply_random_resource_assignment,
        write_verified_content,
    };
    use crate::features::garages::models::{
        AiDriverMutationOperation, GarageOperation, GarageOwnership, GarageSize,
    };
    use crate::features::garages::parser::{
        parse_ai_driver_management_from_sii, parse_garages_from_sii,
    };
    use crate::features::garages::validator::{
        GarageAssignmentVerificationSpec, GarageVerificationSpec, verify_ai_driver_consistency,
        verify_ai_driver_mutation, verify_garage_mutation, verify_garage_purchase_batch,
        verify_garage_relinquishment_batch, verify_garage_resource_assignment,
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
        let sample = SAMPLE.replace("\r\n", "\n");
        let truck = sample.replace(
            "garage : garage.paris {\n vehicles: 3\n vehicles[0]: null",
            "garage : garage.paris {\n vehicles: 3\n vehicles[0]: truck.one",
        );
        assert_eq!(
            apply_garage_relinquishment(&truck, "garage.paris").unwrap_err(),
            "garage_relinquish_not_empty"
        );

        let driver = sample.replace("garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: null", "garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: driver.one");
        assert_eq!(
            apply_garage_relinquishment(&driver, "garage.paris").unwrap_err(),
            "garage_relinquish_not_empty"
        );

        let trailer = sample.replace("garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: null\n drivers[1]: null\n drivers[2]: null\n trailers: 0", "garage : garage.paris {\n vehicles: 3\n vehicles[0]: null\n vehicles[1]: null\n vehicles[2]: null\n drivers: 3\n drivers[0]: null\n drivers[1]: null\n drivers[2]: null\n trailers: 1\n trailers[0]: trailer.one");
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
        let normalized = SAMPLE.replace("\r\n", "\n");
        let mut content = normalized
            .replace(
                " garages[3]: garage.unknown_city\n}",
                " garages[3]: garage.unknown_city\n drivers_offer: 3\n drivers_offer[0]: driver.free_a\n drivers_offer[1]: driver.free_b\n drivers_offer[2]: driver.free_c\n driver_pool: 0\n}",
            )
            .replace(
                "player : _player {\n hq_city: berlin\n}",
                "player : _player {\n hq_city: berlin\n assigned_truck: null\n my_truck: null\n trucks: 6\n trucks[0]: truck.one\n trucks[1]: truck.two\n trucks[2]: truck.paris\n trucks[3]: truck.free_a\n trucks[4]: truck.free_b\n trucks[5]: truck.free_c\n dismissed_drivers: 0\n drivers: 1\n drivers[0]: driver.one\n driver_flags: 1\n driver_flags[0]: 0\n driver_readiness_timer: 1\n driver_readiness_timer[0]: 0\n driver_undrivable_truck_timers: 1\n driver_undrivable_truck_timers[0]: 0\n}",
            )
            .replace("driver_ai : driver.one {\n}", "driver_ai : driver.one {\n assigned_truck: truck.one\n}")
            .replacen(" vehicles[2]: null", " vehicles[2]: truck.free_a", 1)
            .replacen(" vehicles[3]: null", " vehicles[3]: truck.free_b", 1)
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
        assignment_sample()
            .replace(" vehicles[1]: truck.two", " vehicles[1]: null")
            .replace(" vehicles[2]: truck.free_a", " vehicles[2]: null")
            .replace(" vehicles[3]: truck.free_b", " vehicles[3]: null")
            .replace(
            " trucks: 6\n trucks[0]: truck.one\n trucks[1]: truck.two\n trucks[2]: truck.paris\n trucks[3]: truck.free_a\n trucks[4]: truck.free_b\n trucks[5]: truck.free_c",
            " trucks: 3\n trucks[0]: truck.one\n trucks[1]: truck.two\n trucks[2]: truck.paris",
        )
    }

    fn assignment_sample_without_available_drivers() -> String {
        assignment_sample()
            .replace(
                " drivers_offer: 3\n drivers_offer[0]: driver.free_a\n drivers_offer[1]: driver.free_b\n drivers_offer[2]: driver.free_c",
                " drivers_offer: 0",
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
        assert!(matches!(plan.assigned_driver_slot_index, Some(1 | 2)));
        assert_eq!(paris.assigned_driver_count, 1);
        assert_eq!(paris.assigned_truck_count, 1);
        assert_eq!(paris.slots[0].truck_id.as_deref(), Some("truck.paris"));
        let driver_slot = plan.assigned_driver_slot_index.unwrap();
        assert_eq!(paris.slots[driver_slot].driver_id, plan.assigned_driver_id);
        assert!(paris.slots[driver_slot].truck_id.is_none());
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
        assert!(driver_error.starts_with("ai_driver_insufficient_garage_capacity"));
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
        assert!(driver_error.starts_with("ai_driver_insufficient_candidates"));
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

    #[test]
    fn ai_driver_batch_generation_truck_changes_and_removal_stay_consistent() {
        let before = assignment_sample();
        verify_ai_driver_consistency(&before).unwrap();
        let generated = apply_ai_driver_generation(&before, "garage.paris", 2, false).unwrap();
        verify_ai_driver_mutation(
            &before,
            &generated.content,
            &AiDriverMutationOperation::Generate,
            &generated.affected_driver_ids,
        )
        .unwrap();
        assert_eq!(generated.affected_driver_ids.len(), 2);

        let driver_id = generated.affected_driver_ids[0].clone();
        let truck_id = parse_ai_driver_management_from_sii(&generated.content, "ets2", "")
            .unwrap()
            .available_trucks
            .first()
            .unwrap()
            .truck_id
            .clone();
        let assigned =
            apply_ai_driver_truck_update(&generated.content, &driver_id, Some(&truck_id)).unwrap();
        verify_ai_driver_mutation(
            &generated.content,
            &assigned.content,
            &AiDriverMutationOperation::AssignTruck,
            &assigned.affected_driver_ids,
        )
        .unwrap();
        let assigned_driver = parse_ai_driver_management_from_sii(&assigned.content, "ets2", "")
            .unwrap()
            .drivers
            .into_iter()
            .find(|driver| driver.driver_id == driver_id)
            .unwrap();
        assert_eq!(assigned_driver.truck_id.as_deref(), Some(truck_id.as_str()));

        let unassigned = apply_ai_driver_truck_update(&assigned.content, &driver_id, None).unwrap();
        verify_ai_driver_mutation(
            &assigned.content,
            &unassigned.content,
            &AiDriverMutationOperation::UnassignTruck,
            &unassigned.affected_driver_ids,
        )
        .unwrap();
        let removed =
            apply_ai_driver_removal(&unassigned.content, &generated.affected_driver_ids).unwrap();
        verify_ai_driver_mutation(
            &unassigned.content,
            &removed.content,
            &AiDriverMutationOperation::Remove,
            &removed.affected_driver_ids,
        )
        .unwrap();
        verify_ai_driver_consistency(&removed.content).unwrap();
    }

    #[test]
    fn ai_driver_removal_rejects_active_job() {
        let before = assignment_sample_without_available_drivers().replace(
            " drivers_offer: 0",
            " drivers_offer: 1\n drivers_offer[0]: driver.free_a",
        );
        let generated = apply_ai_driver_generation(&before, "garage.paris", 1, false).unwrap();
        let driver_id = generated.affected_driver_ids[0].clone();
        let active = generated.content.replace(
            &format!("driver_ai : {driver_id} {{\n assigned_truck: null"),
            &format!("driver_ai : {driver_id} {{\n state: 2\n assigned_truck: null"),
        );
        let error = apply_ai_driver_removal(&active, &[driver_id.clone()]).unwrap_err();
        assert_eq!(error, format!("ai_driver_active_job:{driver_id}"));
    }

    #[test]
    fn real_fixture_ai_driver_generation_and_removal_preserve_unit_inventory() {
        verify_ai_driver_consistency(REAL_SAMPLE).unwrap();
        let parsed = parse_garages_from_sii(REAL_SAMPLE).unwrap();
        let target = parsed
            .garages
            .iter()
            .find(|garage| {
                garage.ownership == GarageOwnership::Owned
                    && garage
                        .slots
                        .iter()
                        .filter(|slot| slot.driver_id.is_none() && slot.truck_id.is_some())
                        .count()
                        >= 2
            })
            .unwrap();
        let generated =
            apply_ai_driver_generation(REAL_SAMPLE, &target.garage_id, 2, true).unwrap();
        verify_ai_driver_mutation(
            REAL_SAMPLE,
            &generated.content,
            &AiDriverMutationOperation::Generate,
            &generated.affected_driver_ids,
        )
        .unwrap();
        assert_eq!(
            save_unit_counts(REAL_SAMPLE),
            save_unit_counts(&generated.content)
        );

        let removed =
            apply_ai_driver_removal(&generated.content, &generated.affected_driver_ids).unwrap();
        verify_ai_driver_mutation(
            &generated.content,
            &removed.content,
            &AiDriverMutationOperation::Remove,
            &removed.affected_driver_ids,
        )
        .unwrap();
        assert_eq!(
            save_unit_counts(REAL_SAMPLE),
            save_unit_counts(&removed.content)
        );
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
