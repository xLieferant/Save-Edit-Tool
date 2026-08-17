use std::collections::{HashMap, HashSet};

use crate::features::truck_change::parser::{
    UnitBlock, extract_array_entries, extract_field_value, is_null_ref, normalize_sii_unit_id,
    parse_unit_blocks,
};

use super::models::{AiDriverPoolDiagnostics, AiDriverPoolEntry, AiDriverPoolSnapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAiDriverPool {
    pub drivers: Vec<AiDriverPoolEntry>,
    pub available_driver_ids: Vec<String>,
    pub assigned_driver_ids: Vec<String>,
    pub diagnostics: AiDriverPoolDiagnostics,
}

pub fn parse_ai_driver_pool(content: &str) -> Result<ParsedAiDriverPool, String> {
    parse_ai_driver_pool_from_blocks(&parse_unit_blocks(content))
}

pub fn ai_driver_pool_snapshot(
    content: &str,
    profile_id: String,
    save_id: String,
    save_hash: String,
) -> Result<AiDriverPoolSnapshot, String> {
    let parsed = parse_ai_driver_pool(content)?;
    Ok(AiDriverPoolSnapshot {
        profile_id,
        save_id,
        save_hash,
        driver_pool_count: parsed.drivers.len(),
        available_driver_count: parsed.available_driver_ids.len(),
        assigned_driver_count: parsed.assigned_driver_ids.len(),
        drivers: parsed.drivers,
        available_driver_ids: parsed.available_driver_ids,
        assigned_driver_ids: parsed.assigned_driver_ids,
        diagnostics: parsed.diagnostics,
    })
}

pub fn parse_ai_driver_pool_from_blocks(
    blocks: &[UnitBlock],
) -> Result<ParsedAiDriverPool, String> {
    let economy_block = unique_block(blocks, "economy")
        .ok_or_else(|| "driver_pool_invalid:economy_missing".to_string())?;
    let declared_count = extract_field_value(&economy_block.raw_block, "driver_pool")
        .ok_or_else(|| "driver_pool_invalid:count_missing".to_string())?
        .parse::<usize>()
        .map_err(|_| "driver_pool_invalid:count_invalid".to_string())?;
    let raw_entries = extract_array_entries(&economy_block.raw_block, "driver_pool");
    let mut warnings = Vec::new();
    if raw_entries.len() != declared_count {
        warnings.push(format!(
            "driver_pool_count_mismatch:declared={declared_count}:parsed={}",
            raw_entries.len()
        ));
    }
    let unique_indices = raw_entries
        .iter()
        .map(|(index, _)| *index)
        .collect::<HashSet<_>>();
    if unique_indices.len() != raw_entries.len()
        || raw_entries
            .iter()
            .any(|(index, _)| *index >= declared_count)
    {
        warnings.push("driver_pool_indices_invalid".to_string());
    }

    let mut seen = HashSet::new();
    let mut drivers = Vec::new();
    let mut duplicate_driver_count = 0usize;
    for (index, driver_id) in &raw_entries {
        if is_null_ref(driver_id) {
            warnings.push(format!("driver_pool_null_entry:{index}"));
            continue;
        }
        let normalized = normalize_sii_unit_id(driver_id);
        if normalized.is_empty() {
            warnings.push(format!("driver_pool_invalid_entry:{index}"));
            continue;
        }
        if !seen.insert(normalized.clone()) {
            duplicate_driver_count += 1;
            warnings.push(format!("driver_pool_duplicate_driver:{normalized}"));
            continue;
        }
        drivers.push(AiDriverPoolEntry {
            index: *index,
            driver_id: normalized,
        });
    }

    let driver_block_counts = driver_ai_block_counts(blocks);
    let assigned_driver_ids = assigned_driver_ids(blocks);
    let assigned_set = assigned_driver_ids
        .iter()
        .cloned()
        .collect::<HashSet<String>>();
    let mut missing_driver_block_count = 0usize;
    let mut available_driver_ids = Vec::new();
    for entry in &drivers {
        match driver_block_counts
            .get(&entry.driver_id)
            .copied()
            .unwrap_or(0)
        {
            0 => {
                missing_driver_block_count += 1;
                warnings.push(format!(
                    "driver_pool_driver_block_missing:{}",
                    entry.driver_id
                ));
            }
            1 if !assigned_set.contains(&entry.driver_id) => {
                available_driver_ids.push(entry.driver_id.clone());
            }
            1 => {}
            _ => warnings.push(format!(
                "driver_pool_driver_block_ambiguous:{}",
                entry.driver_id
            )),
        }
    }

    Ok(ParsedAiDriverPool {
        diagnostics: AiDriverPoolDiagnostics {
            declared_count,
            parsed_entry_count: raw_entries.len(),
            unique_driver_count: drivers.len(),
            duplicate_driver_count,
            assigned_driver_count: assigned_driver_ids.len(),
            available_driver_count: available_driver_ids.len(),
            missing_driver_block_count,
            warnings,
        },
        drivers,
        available_driver_ids,
        assigned_driver_ids,
    })
}

pub fn assigned_driver_ids(blocks: &[UnitBlock]) -> Vec<String> {
    let mut assigned = Vec::new();
    let mut seen = HashSet::new();
    for block in blocks.iter().filter(|block| block.unit_type == "garage") {
        for (_, driver_id) in extract_array_entries(&block.raw_block, "drivers") {
            if is_null_ref(&driver_id) {
                continue;
            }
            let normalized = normalize_sii_unit_id(&driver_id);
            if !normalized.is_empty() && seen.insert(normalized.clone()) {
                assigned.push(normalized);
            }
        }
    }
    if let Some(player_block) = player_block(blocks) {
        for (_, driver_id) in extract_array_entries(&player_block.raw_block, "drivers") {
            if is_null_ref(&driver_id) {
                continue;
            }
            let normalized = normalize_sii_unit_id(&driver_id);
            if !normalized.is_empty() && seen.insert(normalized.clone()) {
                assigned.push(normalized);
            }
        }
    }
    assigned.sort();
    assigned
}

pub fn driver_ai_block_counts(blocks: &[UnitBlock]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for block in blocks.iter().filter(|block| block.unit_type == "driver_ai") {
        let normalized = normalize_sii_unit_id(&block.id);
        if normalized.is_empty() {
            continue;
        }
        *counts.entry(normalized).or_insert(0) += 1;
    }
    counts
}

pub fn economy_block(blocks: &[UnitBlock]) -> Result<UnitBlock, String> {
    unique_block(blocks, "economy")
        .cloned()
        .ok_or_else(|| "driver_pool_invalid:economy_missing".to_string())
}

pub fn player_block(blocks: &[UnitBlock]) -> Option<UnitBlock> {
    let economy_block = unique_block(blocks, "economy")?;
    let player_id = extract_field_value(&economy_block.raw_block, "player")?;
    blocks
        .iter()
        .find(|block| block.unit_type == "player" && block.id == player_id)
        .cloned()
}

fn unique_block<'a>(blocks: &'a [UnitBlock], unit_type: &str) -> Option<&'a UnitBlock> {
    let matching = blocks
        .iter()
        .filter(|block| block.unit_type == unit_type)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [block] => Some(*block),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ai_driver_pool;

    const SAMPLE: &str = r#"SiiNunit
{
economy : _economy {
 player: _player
 garages: 1
 garages[0]: garage.berlin
 driver_pool: 4
 driver_pool[0]: driver.free_a
 driver_pool[1]: driver.free_b
 driver_pool[2]: driver.free_a
 driver_pool[3]: driver.missing
}
player : _player {
 drivers: 1
 drivers[0]: driver.hired
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
 drivers[0]: driver.hired
 drivers[1]: null
 drivers[2]: null
 trailers: 0
 status: 2
 profit_log: profit.berlin
 productivity: 0
}
profit_log : profit.berlin {
}
driver_ai : driver.hired {
 assigned_truck: null
}
driver_ai : driver.free_a {
 assigned_truck: null
}
driver_ai : driver.free_b {
 assigned_truck: null
}
}
"#;

    #[test]
    fn parses_driver_pool_and_deduplicates_ids() {
        let parsed = parse_ai_driver_pool(SAMPLE).unwrap();
        assert_eq!(
            parsed
                .drivers
                .iter()
                .map(|entry| entry.driver_id.as_str())
                .collect::<Vec<_>>(),
            vec!["driver.free_a", "driver.free_b", "driver.missing"]
        );
        assert_eq!(
            parsed.available_driver_ids,
            vec!["driver.free_a", "driver.free_b"]
        );
        assert_eq!(parsed.assigned_driver_ids, vec!["driver.hired"]);
        assert_eq!(parsed.diagnostics.duplicate_driver_count, 1);
        assert_eq!(parsed.diagnostics.missing_driver_block_count, 1);
    }

    #[test]
    fn parses_real_fixture_driver_pool() {
        let parsed = parse_ai_driver_pool(include_str!(
            "../../../test-fixtures/decrypt/plain_game.sii"
        ))
        .unwrap();
        assert_eq!(parsed.diagnostics.declared_count, 283);
        assert_eq!(parsed.drivers.len(), 283);
        assert!(parsed.available_driver_ids.len() > 250);
    }
}
