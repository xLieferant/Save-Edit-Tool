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
    if level >= 100 {
        return 0;
    }
    if level <= 9 {
        let increment = (level.saturating_sub(1) as u64) * 500;
        return 1500 + increment;
    }
    if level <= 49 {
        return 2500;
    }
    7500
}

pub fn total_xp_to_reach_level(level: u32) -> u64 {
    if level <= 1 {
        return 0;
    }
    let capped = level.min(100);
    let mut total: u64 = 0;
    let mut current_level: u32 = 1;
    while current_level < capped {
        total = total.saturating_add(xp_required_for_level(current_level));
        current_level = current_level.saturating_add(1);
    }
    total
}

pub fn calculate_level(total_xp: u64) -> LevelProgress {
    let mut level: u32 = 1;
    let mut remaining: u64 = total_xp;
    while level < 100 {
        let required = xp_required_for_level(level);
        if remaining < required {
            break;
        }
        remaining = remaining.saturating_sub(required);
        level = level.saturating_add(1);
    }

    let xp_needed_for_next_level = xp_required_for_level(level);
    let xp_into_level = remaining;
    let progress_percent = if xp_needed_for_next_level == 0 {
        100.0
    } else {
        (xp_into_level as f32 / xp_needed_for_next_level as f32) * 100.0
    };

    LevelProgress {
        level,
        current_xp: total_xp,
        xp_into_level,
        xp_needed_for_next_level,
        progress_percent,
    }
}
