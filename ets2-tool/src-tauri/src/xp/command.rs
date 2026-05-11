use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobType {
    Normal,
    Special,
    Htc,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelProgress {
    pub level: u32,
    pub current_xp: u64,
    pub xp_into_level: u64,
    pub xp_needed_for_next_level: u64,
    pub progress_percent: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct LevelTableEntry {
    level: u32,
    increase: u64,
    total_xp: u64,
}

fn level_table() -> &'static [LevelTableEntry] {
    static LEVEL_TABLE: OnceLock<Vec<LevelTableEntry>> = OnceLock::new();

    LEVEL_TABLE.get_or_init(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../src/data/level-table.json"
        )))
        .expect("level table must be valid JSON")
    })
}

fn max_level() -> u32 {
    level_table().last().map(|entry| entry.level).unwrap_or(0)
}

pub fn calculate_xp(distance_km: u32, job_type: JobType, reverse_parked: bool) -> u32 {
    let multiplier = match job_type {
        JobType::Normal => 1,
        JobType::Special => 3,
        JobType::Htc => 2,
    };
    let mut xp = distance_km.saturating_mul(multiplier);
    if reverse_parked {
        xp = xp.saturating_add(50);
    }
    xp
}

pub fn xp_required_for_level(level: u32) -> u64 {
    let capped_level = level.min(max_level());
    if capped_level >= max_level() {
        return 0;
    }

    level_table()
        .iter()
        .find(|entry| entry.level == capped_level)
        .map(|entry| entry.increase)
        .unwrap_or(0)
}

pub fn total_xp_to_reach_level(level: u32) -> u64 {
    let capped_level = level.min(max_level());

    level_table()
        .iter()
        .find(|entry| entry.level == capped_level)
        .map(|entry| entry.total_xp)
        .unwrap_or(0)
}

pub fn calculate_level(total_xp: u64) -> LevelProgress {
    let table = level_table();
    let max_total_xp = total_xp_to_reach_level(max_level());
    let clamped_xp = total_xp.min(max_total_xp);

    let current_entry = table
        .iter()
        .rev()
        .find(|entry| entry.total_xp <= clamped_xp)
        .copied()
        .unwrap_or(LevelTableEntry {
            level: 0,
            increase: 0,
            total_xp: 0,
        });

    let level = current_entry.level;
    let next_total_xp = table
        .iter()
        .find(|entry| entry.level == level.saturating_add(1))
        .map(|entry| entry.total_xp)
        .unwrap_or(clamped_xp);
    let xp_needed_for_next_level = next_total_xp.saturating_sub(current_entry.total_xp);
    let xp_into_level = clamped_xp.saturating_sub(current_entry.total_xp);
    let progress_percent = if xp_needed_for_next_level == 0 {
        100.0
    } else {
        (xp_into_level as f32 / xp_needed_for_next_level as f32) * 100.0
    };

    LevelProgress {
        level,
        current_xp: clamped_xp,
        xp_into_level,
        xp_needed_for_next_level,
        progress_percent,
    }
}
