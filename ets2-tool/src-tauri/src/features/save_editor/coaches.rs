use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::features::truck_change::parser::{
    UnitBlock, extract_array_entries, extract_field_value, is_null_ref, normalize_sii_unit_id,
    parse_unit_blocks,
};
use crate::features::truck_change::writer::set_unit_field_value;

pub const BUS_STAT_FIELDS: [&str; 8] = [
    "bus_experience_points",
    "bus_total_distance",
    "bus_finished_job_count",
    "bus_cancelled_job_count",
    "bus_total_passengers",
    "bus_total_stops",
    "bus_game_time",
    "bus_playing_time",
];

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Career {
    Truck,
    Bus,
}

impl Career {
    pub fn xp_field(self) -> &'static str {
        match self {
            Self::Truck => "experience_points",
            Self::Bus => "bus_experience_points",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusStop {
    pub id: String,
    pub resolved: bool,
    pub discovered: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusJobLog {
    pub id: String,
    pub version: Option<String>,
    pub entries: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoachesSnapshot {
    pub save_hash: String,
    pub truck_xp: Option<i64>,
    pub bus_xp: Option<i64>,
    pub statistics: std::collections::BTreeMap<String, Option<String>>,
    pub stops: Vec<BusStop>,
    pub discovered_stops: usize,
    pub declared_stop_count: Option<String>,
    pub buses: Option<String>,
    pub bus_references: Vec<BusStop>,
    pub current_bus_job: Option<String>,
    pub current_bus_job_resolved: bool,
    pub my_vehicles_mode: Option<String>,
    pub assigned_vehicles_mode: Option<String>,
    pub job_logs: Vec<BusJobLog>,
    pub warnings: Vec<String>,
}

pub fn save_hash(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

fn unique_unit<'a>(blocks: &'a [UnitBlock], kind: &str) -> Result<&'a UnitBlock, String> {
    let matches = blocks
        .iter()
        .filter(|block| block.unit_type == kind)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [unit] => Ok(unit),
        [] => Err(format!("coaches_unit_missing:{kind}")),
        _ => Err(format!("coaches_unit_ambiguous:{kind}")),
    }
}

fn resolve<'a>(blocks: &'a [UnitBlock], id: &str) -> Option<&'a UnitBlock> {
    let id = normalize_sii_unit_id(id);
    let mut matches = blocks
        .iter()
        .filter(|block| normalize_sii_unit_id(&block.id) == id);
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}

fn xp(unit: &UnitBlock, field: &str) -> Option<i64> {
    extract_field_value(&unit.raw_block, field)?
        .parse::<i64>()
        .ok()
        .filter(|value| *value >= 0)
}

pub fn read_snapshot(content: &str) -> Result<CoachesSnapshot, String> {
    let blocks = parse_unit_blocks(content);
    let economy = unique_unit(&blocks, "economy")?;
    let player = match extract_field_value(&economy.raw_block, "player") {
        Some(id) => resolve(&blocks, &id).filter(|unit| unit.unit_type == "player"),
        None => unique_unit(&blocks, "player").ok(),
    };
    let player_field = |field| player.and_then(|unit| extract_field_value(&unit.raw_block, field));
    let mut warnings = Vec::new();
    let stop_entries = extract_array_entries(&economy.raw_block, "bus_stops");
    let declared_stop_count = extract_field_value(&economy.raw_block, "bus_stops");
    if declared_stop_count
        .as_deref()
        .and_then(|value| value.parse::<usize>().ok())
        != Some(stop_entries.len())
        && (declared_stop_count.is_some() || !stop_entries.is_empty())
    {
        warnings.push("coaches.stop_array_invalid".to_string());
    }
    if stop_entries
        .iter()
        .enumerate()
        .any(|(expected, (actual, _))| expected != *actual)
    {
        warnings.push("coaches.stop_array_invalid".to_string());
    }
    let mut seen = std::collections::HashSet::new();
    let stops = stop_entries
        .into_iter()
        .filter_map(|(_, id)| {
            if !seen.insert(normalize_sii_unit_id(&id)) {
                warnings.push("coaches.stop_array_invalid".to_string());
                return None;
            }
            let stop = resolve(&blocks, &id).filter(|unit| unit.unit_type == "bus_stop");
            let discovered = stop
                .and_then(|unit| extract_field_value(&unit.raw_block, "discovered"))
                .and_then(|value| match value.as_str() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                });
            if stop.is_none() || discovered.is_none() {
                warnings.push("coaches.stop_unresolved".to_string());
            }
            Some(BusStop {
                id,
                resolved: stop.is_some(),
                discovered,
            })
        })
        .collect::<Vec<_>>();
    let buses = player_field("buses");
    let bus_entries = player
        .map(|unit| extract_array_entries(&unit.raw_block, "buses"))
        .unwrap_or_default();
    if buses
        .as_deref()
        .and_then(|value| value.parse::<usize>().ok())
        != Some(bus_entries.len())
        && (buses.is_some() || !bus_entries.is_empty())
    {
        warnings.push("coaches.bus_array_invalid".to_string());
    }
    let bus_references = bus_entries
        .into_iter()
        .map(|(_, id)| {
            let resolved = resolve(&blocks, &id).is_some();
            BusStop {
                id,
                resolved,
                discovered: None,
            }
        })
        .collect();
    let current_bus_job = player_field("current_bus_job");
    let current_bus_job_resolved = current_bus_job
        .as_deref()
        .filter(|value| !is_null_ref(value))
        .and_then(|id| resolve(&blocks, id))
        .is_some();
    let job_logs = blocks
        .iter()
        .filter(|unit| unit.unit_type == "bus_job_log")
        .map(|unit| BusJobLog {
            id: unit.id.clone(),
            version: extract_field_value(&unit.raw_block, "version"),
            entries: extract_field_value(&unit.raw_block, "entries"),
        })
        .collect();
    warnings.sort();
    warnings.dedup();
    Ok(CoachesSnapshot {
        save_hash: save_hash(content),
        truck_xp: xp(economy, Career::Truck.xp_field()),
        bus_xp: xp(economy, Career::Bus.xp_field()),
        statistics: BUS_STAT_FIELDS
            .iter()
            .map(|field| {
                (
                    field.to_string(),
                    extract_field_value(&economy.raw_block, field),
                )
            })
            .collect(),
        discovered_stops: stops
            .iter()
            .filter(|stop| stop.discovered == Some(true))
            .count(),
        declared_stop_count,
        stops,
        buses,
        bus_references,
        current_bus_job,
        current_bus_job_resolved,
        my_vehicles_mode: player_field("my_vehicles_mode"),
        assigned_vehicles_mode: player_field("assigned_vehicles_mode"),
        job_logs,
        warnings,
    })
}

/// Uses the existing SII unit parser/writer; never reconstructs a save from a DTO.
pub fn patch_xp(content: &str, career: Career, value: i64) -> Result<String, String> {
    if !(0..=9_007_199_254_740_991).contains(&value) {
        return Err("coaches.xp_invalid".to_string());
    }
    let blocks = parse_unit_blocks(content);
    let economy = unique_unit(&blocks, "economy")?;
    if resolve(&blocks, &economy.id).is_none() {
        return Err("coaches_unit_ambiguous:economy".to_string());
    }
    let field = career.xp_field();
    if xp(economy, field).is_none() {
        return Err("coaches.xp_missing".to_string());
    }
    let occurrences = economy
        .raw_block
        .lines()
        .filter(|line| line.trim_start().starts_with(&format!("{field}:")))
        .count();
    if occurrences != 1 {
        return Err("coaches.xp_invalid".to_string());
    }
    let (mut updated, changed) =
        set_unit_field_value(content, &economy.id, field, &value.to_string())?;
    if !changed {
        return Err("coaches.xp_missing".to_string());
    }
    // Preserve the original CRLF convention after the existing line-based writer.
    if content.contains("\r\n") && !content.replace("\r\n", "").contains('\n') {
        updated = updated.replace('\n', "\r\n");
    }
    verify_xp(&updated, career, value)?;
    Ok(updated)
}

pub fn verify_xp(content: &str, career: Career, expected: i64) -> Result<(), String> {
    let blocks = parse_unit_blocks(content);
    let economy = unique_unit(&blocks, "economy")?;
    if xp(economy, career.xp_field()) != Some(expected) {
        return Err("coaches.verify_failed".to_string());
    }
    Ok(())
}
