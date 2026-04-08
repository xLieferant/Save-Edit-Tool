use std::fmt::Write;

use anyhow::{Context, Result, anyhow};
use uuid::Uuid;

use crate::features::ets2save::models::{
    ActiveJobData as BackendActiveJobData, Ets2JobState as BackendEts2JobState,
    JobOfferData as BackendJobInfoData,
};
use crate::features::ets2save::parser::{
    UnitRange, find_player_block_range, parse_player_job_state,
};
use crate::features::ets2save::sii_codec::split_lines;

pub type JobInfoData = BackendJobInfoData;
pub type ActiveJobData = BackendActiveJobData;
pub type Ets2JobState = BackendEts2JobState;

#[derive(Debug, Clone)]
pub struct PlayerBlockData {
    pub id: String,
    pub range: UnitRange,
    pub assigned_truck: Option<String>,
    pub assigned_trailer: Option<String>,
    pub my_truck: Option<String>,
    pub my_trailer: Option<String>,
    pub selected_job: Option<String>,
    pub current_job: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlayerJobRefState {
    pub player_block: PlayerBlockData,
    pub selected_job: Option<JobInfoData>,
    pub active_job: Option<ActiveJobData>,
    pub job_state: Ets2JobState,
}

impl PlayerJobRefState {
    pub fn quick_assessment(&self) -> &'static str {
        match &self.job_state {
            Ets2JobState::None => "no job",
            Ets2JobState::Selected(_) => "selected job only",
            Ets2JobState::Active(_) => "active accepted job",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    Lenient,
    Strict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatcherJobKind {
    QuickJob,
    FreightMarketOwnTruck { trailer_required: bool },
    ExternalContractPlaceholder,
}

#[derive(Debug, Clone)]
pub struct DispatcherJobDraft {
    pub cargo: String,
    pub source_company: String,
    pub target_company: String,
    pub planned_distance_km: Option<i64>,
    pub urgency: Option<i64>,
    pub ferry_time: Option<i64>,
    pub ferry_price: Option<i64>,
    pub units_count: Option<i64>,
    pub fill_ratio: Option<i64>,
    pub is_articulated: Option<bool>,
    pub is_cargo_market_job: Option<bool>,
    pub total_fines: Option<i64>,
    pub time_lower_limit: Option<i64>,
    pub time_upper_limit: Option<i64>,
    pub start_time: Option<i64>,
    pub autoload_used: Option<bool>,
    pub is_trailer_loaded: Option<bool>,
    pub company_truck: Option<String>,
    pub company_trailer: Option<String>,
    pub player_truck: Option<String>,
    pub player_trailer: Option<String>,
    pub kind: DispatcherJobKind,
}

impl DispatcherJobDraft {
    pub fn job_info_pointer(&self) -> String {
        format!("_nameless.{}", Uuid::new_v4().as_u128())
    }

    pub fn player_job_pointer(&self) -> String {
        self.job_info_pointer()
    }

    pub fn validate(
        &self,
        player_block: &PlayerBlockData,
        lines: &[String],
        mode: ValidationMode,
    ) -> Result<()> {
        if self.cargo.trim().is_empty() {
            return Err(anyhow!("cargo missing in job draft"));
        }
        if self.source_company.trim().is_empty() || self.target_company.trim().is_empty() {
            return Err(anyhow!("company references missing in job draft"));
        }
        if self.source_company == self.target_company {
            return Err(anyhow!("source and target company must differ"));
        }

        match &self.kind {
            DispatcherJobKind::QuickJob => {
                if self.company_truck.is_none() || self.company_trailer.is_none() {
                    return Err(anyhow!(
                        "quick jobs must provide both company truck and trailer pointers"
                    ));
                }
            }
            DispatcherJobKind::FreightMarketOwnTruck { trailer_required } => {
                if player_block.assigned_truck.is_none() {
                    return Err(anyhow!("assigned truck missing for own-truck job"));
                }
                if *trailer_required && player_block.assigned_trailer.is_none() {
                    return Err(anyhow!("assigned trailer missing for own-truck job"));
                }
            }
            DispatcherJobKind::ExternalContractPlaceholder => {}
        }

        if mode == ValidationMode::Strict {
            if let Some(pointer) = self
                .company_truck
                .as_deref()
                .or(self.player_truck.as_deref())
            {
                if !block_exists(lines, "truck", pointer) {
                    return Err(anyhow!("referenced truck block {} missing", pointer));
                }
            }
            if let Some(pointer) = self
                .company_trailer
                .as_deref()
                .or(self.player_trailer.as_deref())
            {
                if !block_exists(lines, "trailer", pointer) {
                    return Err(anyhow!("referenced trailer block {} missing", pointer));
                }
            }
        }

        Ok(())
    }
}

fn block_exists(lines: &[String], block_type: &str, pointer: &str) -> bool {
    let token = format!("{} : {}", block_type, pointer);
    find_block_range(lines, &token).is_some()
}

pub fn parse_player_id(game_sii: &str) -> Result<String> {
    let lines = split_lines(game_sii);
    for line in lines {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("player :") {
            let candidate = rest
                .trim()
                .split_whitespace()
                .next()
                .ok_or_else(|| anyhow!("failed to parse player id from line `{}`", trimmed))?;
            let id = candidate.trim_end_matches('{').trim();
            if !id.is_empty() {
                return Ok(id.to_string());
            }
        }
    }
    Err(anyhow!("player block not found"))
}

pub fn parse_player_block(game_sii: &str, player_id: &str) -> Result<PlayerBlockData> {
    let lines = split_lines(game_sii);
    let token = format!("player : {}", player_id);
    let range = find_block_range(&lines, &token).context("player block not found")?;
    Ok(PlayerBlockData {
        id: player_id.to_string(),
        range,
        assigned_truck: extract_field(&lines, range, "assigned_truck"),
        assigned_trailer: extract_field(&lines, range, "assigned_trailer"),
        my_truck: extract_field(&lines, range, "my_truck"),
        my_trailer: extract_field(&lines, range, "my_trailer"),
        selected_job: extract_field(&lines, range, "selected_job"),
        current_job: extract_field(&lines, range, "current_job"),
    })
}

pub fn parse_selected_job_id(player_block: &PlayerBlockData) -> Option<String> {
    player_block
        .selected_job
        .as_ref()
        .map(|value| value.clone())
        .filter(|value| !value.eq_ignore_ascii_case("nil"))
}

pub fn parse_current_job_id(player_block: &PlayerBlockData) -> Option<String> {
    player_block
        .current_job
        .as_ref()
        .map(|value| value.clone())
        .filter(|value| !value.eq_ignore_ascii_case("nil"))
}

pub fn parse_job_info_block(game_sii: &str, job_id: &str) -> Result<JobInfoData> {
    let lines = split_lines(game_sii);
    let token = format!("job_info : {}", job_id);
    let range = find_block_range(&lines, &token)
        .ok_or_else(|| anyhow!("job_info block for {job_id} not found"))?;
    Ok(JobInfoData {
        pointer: job_id.to_string(),
        cargo: extract_field(&lines, range, "cargo"),
        source_company: extract_field(&lines, range, "source_company"),
        target_company: extract_field(&lines, range, "target_company"),
        planned_distance_km: parse_i64_field(&lines, range, "planned_distance_km"),
        urgency: parse_i64_field(&lines, range, "urgency"),
        cargo_model_index: parse_i64_field(&lines, range, "cargo_model_index"),
        is_cargo_market_job: parse_bool_field(&lines, range, "is_cargo_market_job"),
        units_count: parse_i64_field(&lines, range, "units_count"),
        fill_ratio: parse_i64_field(&lines, range, "fill_ratio"),
    })
}

pub fn parse_player_job_block(game_sii: &str, job_id: &str) -> Result<ActiveJobData> {
    let lines = split_lines(game_sii);
    let token = format!("player_job : {}", job_id);
    let range = find_block_range(&lines, &token)
        .ok_or_else(|| anyhow!("player_job block for {job_id} not found"))?;
    Ok(ActiveJobData {
        pointer: job_id.to_string(),
        company_truck: extract_field(&lines, range, "company_truck"),
        company_trailer: extract_field(&lines, range, "company_trailer"),
        cargo: extract_field(&lines, range, "cargo"),
        source_company: extract_field(&lines, range, "source_company"),
        target_company: extract_field(&lines, range, "target_company"),
        planned_distance_km: parse_i64_field(&lines, range, "planned_distance_km"),
        urgency: parse_i64_field(&lines, range, "urgency"),
        total_fines: parse_i64_field(&lines, range, "total_fines"),
        time_lower_limit: parse_i64_field(&lines, range, "time_lower_limit"),
        time_upper_limit: parse_i64_field(&lines, range, "time_upper_limit"),
        start_time: parse_i64_field(&lines, range, "start_time"),
        is_trailer_loaded: parse_bool_field(&lines, range, "is_trailer_loaded"),
        autoload_used: parse_bool_field(&lines, range, "autoload_used"),
        is_cargo_market_job: parse_bool_field(&lines, range, "is_cargo_market_job"),
        selected_target: extract_field(&lines, range, "selected_target"),
        cargo_model_index: parse_i64_field(&lines, range, "cargo_model_index"),
        units_count: parse_i64_field(&lines, range, "units_count"),
        fill_ratio: parse_i64_field(&lines, range, "fill_ratio"),
    })
}

pub fn parse_job_state(game_sii: &str) -> Result<Ets2JobState> {
    let lines = split_lines(game_sii);
    parse_player_job_state(&lines).ok_or_else(|| anyhow!("player job state not parsable"))
}

pub fn inspect_quicksave(game_sii: &str) -> Result<PlayerJobRefState> {
    let player_id = parse_player_id(game_sii)?;
    let player_block = parse_player_block(game_sii, &player_id)?;
    let selected_job = parse_selected_job_id(&player_block)
        .map(|pointer| parse_job_info_block(game_sii, &pointer))
        .transpose()?;
    let active_job = parse_current_job_id(&player_block)
        .map(|pointer| parse_player_job_block(game_sii, &pointer))
        .transpose()?;
    let job_state = parse_job_state(game_sii)?;
    Ok(PlayerJobRefState {
        player_block,
        selected_job,
        active_job,
        job_state,
    })
}

pub fn create_job_info_block(pointer: &str, draft: &DispatcherJobDraft) -> Vec<String> {
    let mut block = Vec::new();
    block.push(format!("job_info : {} {{", pointer));
    block.push(format!(" cargo: {}", draft.cargo));
    block.push(format!(" source_company: {}", draft.source_company));
    block.push(format!(" target_company: {}", draft.target_company));
    if let Some(value) = draft.planned_distance_km {
        block.push(format!(" planned_distance_km: {}", value));
    }
    if let Some(value) = draft.urgency {
        block.push(format!(" urgency: {}", value));
    }
    if let Some(value) = draft.ferry_time {
        block.push(format!(" ferry_time: {}", value));
    }
    if let Some(value) = draft.ferry_price {
        block.push(format!(" ferry_price: {}", value));
    }
    if let Some(value) = draft.units_count {
        block.push(format!(" units_count: {}", value));
    }
    if let Some(value) = draft.fill_ratio {
        block.push(format!(" fill_ratio: {}", value));
    }
    if let Some(true) = draft.is_articulated {
        block.push(" is_articulated: true".to_string());
    }
    if let Some(value) = draft.is_cargo_market_job {
        block.push(format!(" is_cargo_market_job: {}", value));
    }
    if let Some(value) = &draft.company_truck {
        block.push(format!(" company_truck: {}", value));
    }
    if let Some(value) = &draft.company_trailer {
        block.push(format!(" company_trailer: {}", value));
    }
    block.push("}".to_string());
    block
}

pub fn create_player_job_block(
    pointer: &str,
    draft: &DispatcherJobDraft,
    job_info_pointer: Option<&str>,
) -> Vec<String> {
    let mut block = Vec::new();
    block.push(format!("player_job : {} {{", pointer));
    if let Some(job_info_pointer) = job_info_pointer {
        block.push(format!(" job_offer_data: {}", job_info_pointer));
    }
    block.push(format!(" cargo: {}", draft.cargo));
    block.push(format!(" source_company: {}", draft.source_company));
    block.push(format!(" target_company: {}", draft.target_company));
    if let Some(value) = draft.planned_distance_km {
        block.push(format!(" planned_distance_km: {}", value));
    }
    if let Some(value) = draft.urgency {
        block.push(format!(" urgency: {}", value));
    }
    if let Some(value) = draft.total_fines {
        block.push(format!(" total_fines: {}", value));
    }
    if let Some(value) = draft.time_lower_limit {
        block.push(format!(" time_lower_limit: {}", value));
    }
    if let Some(value) = draft.time_upper_limit {
        block.push(format!(" time_upper_limit: {}", value));
    }
    if let Some(value) = draft.start_time {
        block.push(format!(" start_time: {}", value));
    }
    if let Some(value) = draft.is_trailer_loaded {
        block.push(format!(" is_trailer_loaded: {}", value));
    }
    if let Some(value) = draft.autoload_used {
        block.push(format!(" autoload_used: {}", value));
    }
    if let Some(value) = draft.is_cargo_market_job {
        block.push(format!(" is_cargo_market_job: {}", value));
    }
    if let Some(value) = draft
        .company_truck
        .as_deref()
        .or(draft.player_truck.as_deref())
    {
        block.push(format!(" company_truck: {}", value));
    }
    if let Some(value) = draft
        .company_trailer
        .as_deref()
        .or(draft.player_trailer.as_deref())
    {
        block.push(format!(" company_trailer: {}", value));
    }
    block.push("}".to_string());
    block
}

pub fn upsert_job_info_block(lines: &mut Vec<String>, pointer: &str, block: &[String]) {
    upsert_block(lines, "job_info", pointer, block);
}

pub fn upsert_player_job_block(lines: &mut Vec<String>, pointer: &str, block: &[String]) {
    upsert_block(lines, "player_job", pointer, block);
}

fn upsert_block(lines: &mut Vec<String>, block_type: &str, pointer: &str, block: &[String]) {
    let token = format!("{} : {}", block_type, pointer);
    if let Some(range) = find_block_range(lines, &token) {
        lines.splice(range.start..=range.end, block.iter().cloned());
        return;
    }
    lines.push("".to_string());
    lines.extend(block.iter().cloned());
}

pub fn set_player_selected_job(lines: &mut Vec<String>, pointer: &str) -> Result<()> {
    update_player_field(lines, "selected_job", pointer)
}

pub fn set_player_current_job(lines: &mut Vec<String>, pointer: &str) -> Result<()> {
    update_player_field(lines, "current_job", pointer)
}

pub fn clear_player_selected_job(lines: &mut Vec<String>) -> Result<()> {
    update_player_field(lines, "selected_job", "nil")
}

pub fn clear_player_current_job(lines: &mut Vec<String>) -> Result<()> {
    update_player_field(lines, "current_job", "nil")
}

fn update_player_field(lines: &mut Vec<String>, field: &str, value: &str) -> Result<()> {
    let range = find_player_block_range(lines).ok_or_else(|| anyhow!("player block missing"))?;
    for index in range.start..=range.end {
        if lines[index].trim().starts_with(&format!("{field}:")) {
            let indent = lines[index]
                .chars()
                .take_while(|character| character.is_whitespace())
                .collect::<String>();
            lines[index] = format!("{indent}{field}: {value}");
            return Ok(());
        }
    }
    lines.insert(range.end, format!(" {}: {}", field, value));
    Ok(())
}

fn find_block_range(lines: &[String], token: &str) -> Option<UnitRange> {
    let mut start_index = None;
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate() {
        if start_index.is_none() && line.trim().starts_with(token) {
            start_index = Some(index);
        }

        if let Some(start) = start_index {
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;

            if index > start && depth <= 0 {
                return Some(UnitRange { start, end: index });
            }
        }
    }

    None
}

fn parse_i64_field(lines: &[String], range: UnitRange, field: &str) -> Option<i64> {
    extract_field(lines, range, field).and_then(|value| value.parse::<i64>().ok())
}

fn parse_bool_field(lines: &[String], range: UnitRange, field: &str) -> Option<bool> {
    extract_field(lines, range, field).and_then(|value| match value.to_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    })
}

fn extract_field(lines: &[String], range: UnitRange, field: &str) -> Option<String> {
    for index in range.start..=range.end {
        let trimmed = lines[index].trim();
        if trimmed.starts_with(&format!("{field}:")) {
            if let Some((_, value)) = trimmed.split_once(':') {
                let normalized = value.trim();
                if normalized.eq_ignore_ascii_case("nil")
                    || normalized.eq_ignore_ascii_case("null")
                    || normalized.is_empty()
                {
                    return None;
                }
                return Some(normalized.to_string());
            }
        }
    }
    None
}

pub fn debug_dump_job_state(game_sii: &str) -> Result<String> {
    let inspection = inspect_quicksave(game_sii)?;
    let mut output = String::new();
    writeln!(output, "player id: {}", inspection.player_block.id)?;
    writeln!(
        output,
        "assigned truck: {}",
        inspection
            .player_block
            .assigned_truck
            .as_deref()
            .unwrap_or("nil")
    )?;
    writeln!(
        output,
        "assigned trailer: {}",
        inspection
            .player_block
            .assigned_trailer
            .as_deref()
            .unwrap_or("nil")
    )?;
    writeln!(
        output,
        "selected job id: {}",
        inspection
            .player_block
            .selected_job
            .as_deref()
            .unwrap_or("nil")
    )?;
    if let Some(selected) = &inspection.selected_job {
        writeln!(
            output,
            "selected cargo/source/target: {}/{}/{}",
            selected.cargo.as_deref().unwrap_or("nil"),
            selected.source_company.as_deref().unwrap_or("nil"),
            selected.target_company.as_deref().unwrap_or("nil")
        )?;
    }
    writeln!(
        output,
        "current job id: {}",
        inspection
            .player_block
            .current_job
            .as_deref()
            .unwrap_or("nil")
    )?;
    if let Some(active) = &inspection.active_job {
        writeln!(
            output,
            "active cargo/source/target: {}/{}/{}",
            active.cargo.as_deref().unwrap_or("nil"),
            active.source_company.as_deref().unwrap_or("nil"),
            active.target_company.as_deref().unwrap_or("nil")
        )?;
    }
    writeln!(output, "assessment: {}", inspection.quick_assessment())?;
    Ok(output)
}

/// Integration note:
/// - `dispatcher_inspect_quicksave` should pipe the decoded quicksave `game.sii` into `inspect_quicksave`.
/// - `dispatcher_generate_selected_job` should build a `DispatcherJobDraft`, call `create_job_info_block`, and
///   upsert the block before calling `set_player_selected_job`.
/// - `dispatcher_activate_job` should upsert a `player_job` block via `create_player_job_block` and
///   invoke `set_player_current_job`/`clear_player_selected_job` as appropriate.
/// - `dispatcher_clear_job_state` should use `clear_player_selected_job`, `clear_player_current_job`, and
///   remove stale `player_job`/`job_info` blocks by comparing block IDs.

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"SiiNunit
{
player : _nameless.player {
 assigned_truck: _nameless.truck.1
 assigned_trailer: _nameless.trailer.2
 selected_job: nil
 current_job: nil
}
}"#;

    #[test]
    fn parses_player_data() {
        let id = parse_player_id(FIXTURE).unwrap();
        assert_eq!(id, "_nameless.player");
        let block = parse_player_block(FIXTURE, &id).unwrap();
        assert_eq!(
            block.assigned_trailer.as_deref(),
            Some("_nameless.trailer.2")
        );
        assert!(block.selected_job.is_none());
    }

    const SELECTED_JOB_FIXTURE: &str = r#"SiiNunit
{
player : _nameless.player {
 selected_job: _nameless.jobinfo
 current_job: nil
}
job_info : _nameless.jobinfo {
 cargo: cargo.foo
 source_company: company.volatile.src.city
 target_company: company.volatile.dst.city
}
}"#;

    const ACTIVE_JOB_FIXTURE: &str = r#"SiiNunit
{
player : _nameless.player {
 selected_job: nil
 current_job: _nameless.job
}
player_job : _nameless.job {
 cargo: cargo.bar
 source_company: company.volatile.src.city
 target_company: company.volatile.dst.city
}
}"#;

    const NO_TRUCK_FIXTURE: &str = r#"SiiNunit
{
player : _nameless.player {
 assigned_truck: nil
}
}"#;

    #[test]
    fn inspection_reports_none_when_empty() {
        let inspection = inspect_quicksave(FIXTURE).unwrap();
        assert!(matches!(inspection.job_state, Ets2JobState::None));
    }

    #[test]
    fn inspection_reports_selected_job() {
        let inspection = inspect_quicksave(SELECTED_JOB_FIXTURE).unwrap();
        assert!(matches!(inspection.job_state, Ets2JobState::Selected(_)));
        assert_eq!(
            inspection
                .selected_job
                .as_ref()
                .and_then(|job| job.cargo.clone()),
            Some("cargo.foo".to_string())
        );
    }

    #[test]
    fn inspection_reports_active_job() {
        let inspection = inspect_quicksave(ACTIVE_JOB_FIXTURE).unwrap();
        assert!(matches!(inspection.job_state, Ets2JobState::Active(_)));
        assert_eq!(
            inspection
                .player_block
                .current_job
                .as_deref()
                .unwrap_or_default(),
            "_nameless.job"
        );
    }

    #[test]
    fn quick_job_block_contains_fields() {
        let draft = DispatcherJobDraft {
            cargo: "cargo.abc".to_string(),
            source_company: "company.volatile.src.city".to_string(),
            target_company: "company.volatile.dst.city".to_string(),
            planned_distance_km: Some(123),
            urgency: Some(1),
            ferry_time: None,
            ferry_price: None,
            units_count: Some(2),
            fill_ratio: Some(1),
            is_articulated: Some(true),
            is_cargo_market_job: Some(false),
            total_fines: None,
            time_lower_limit: None,
            time_upper_limit: None,
            start_time: None,
            autoload_used: None,
            is_trailer_loaded: None,
            company_truck: Some("_nameless.truck.1".to_string()),
            company_trailer: Some("_nameless.trailer.2".to_string()),
            player_truck: None,
            player_trailer: None,
            kind: DispatcherJobKind::QuickJob,
        };
        let block = create_job_info_block("_nameless.jobinfo", &draft);
        assert!(block.iter().any(|line| line.contains("company_truck")));
    }

    #[test]
    fn player_job_block_mixes_truck_sources() {
        let draft = DispatcherJobDraft {
            cargo: "cargo.abc".to_string(),
            source_company: "company.volatile.src.city".to_string(),
            target_company: "company.volatile.dst.city".to_string(),
            planned_distance_km: None,
            urgency: None,
            ferry_time: None,
            ferry_price: None,
            units_count: None,
            fill_ratio: None,
            is_articulated: None,
            is_cargo_market_job: None,
            total_fines: None,
            time_lower_limit: None,
            time_upper_limit: None,
            start_time: None,
            autoload_used: None,
            is_trailer_loaded: None,
            company_truck: None,
            company_trailer: None,
            player_truck: Some("_nameless.truck.player".to_string()),
            player_trailer: Some("_nameless.trailer.player".to_string()),
            kind: DispatcherJobKind::FreightMarketOwnTruck {
                trailer_required: true,
            },
        };
        let block = create_player_job_block("_nameless.job", &draft, None);
        assert!(
            block
                .iter()
                .any(|line| line.contains("_nameless.truck.player"))
        );
        assert!(
            block
                .iter()
                .any(|line| line.contains("_nameless.trailer.player"))
        );
    }

    #[test]
    fn validation_flags_quick_job_missing_trailer() {
        let id = parse_player_id(FIXTURE).unwrap();
        let block = parse_player_block(FIXTURE, &id).unwrap();
        let draft = DispatcherJobDraft {
            cargo: "cargo.abc".to_string(),
            source_company: "company.volatile.src.city".to_string(),
            target_company: "company.volatile.dst.city".to_string(),
            planned_distance_km: None,
            urgency: None,
            ferry_time: None,
            ferry_price: None,
            units_count: None,
            fill_ratio: None,
            is_articulated: None,
            is_cargo_market_job: None,
            total_fines: None,
            time_lower_limit: None,
            time_upper_limit: None,
            start_time: None,
            autoload_used: None,
            is_trailer_loaded: None,
            company_truck: Some("_nameless.truck.1".to_string()),
            company_trailer: None,
            player_truck: None,
            player_trailer: None,
            kind: DispatcherJobKind::QuickJob,
        };
        let lines = split_lines(FIXTURE);
        assert!(
            draft
                .validate(&block, &lines, ValidationMode::Lenient)
                .is_err()
        );
    }

    #[test]
    fn validation_detects_missing_assigned_truck() {
        let id = parse_player_id(NO_TRUCK_FIXTURE).unwrap();
        let block = parse_player_block(NO_TRUCK_FIXTURE, &id).unwrap();
        let draft = DispatcherJobDraft {
            cargo: "cargo.foo".to_string(),
            source_company: "company.volatile.src".to_string(),
            target_company: "company.volatile.dst".to_string(),
            planned_distance_km: None,
            urgency: None,
            ferry_time: None,
            ferry_price: None,
            units_count: None,
            fill_ratio: None,
            is_articulated: None,
            is_cargo_market_job: None,
            total_fines: None,
            time_lower_limit: None,
            time_upper_limit: None,
            start_time: None,
            autoload_used: None,
            is_trailer_loaded: None,
            company_truck: None,
            company_trailer: None,
            player_truck: None,
            player_trailer: None,
            kind: DispatcherJobKind::FreightMarketOwnTruck {
                trailer_required: false,
            },
        };
        let lines = split_lines(NO_TRUCK_FIXTURE);
        assert!(
            draft
                .validate(&block, &lines, ValidationMode::Lenient)
                .is_err()
        );
    }
}
