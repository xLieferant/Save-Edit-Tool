use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::features::economy::compensation_models::{
    BaseRateType, CargoType, EquipmentType, JobCompensationInput, Urgency,
};
use crate::features::economy::compensation_service;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobLogEntry {
    pub job_id: String,
    pub started_at_utc: String,
    pub ended_at_utc: Option<String>,
    pub origin_city: String,
    pub destination_city: String,
    pub source_company: String,
    pub destination_company: String,
    pub cargo: String,
    pub planned_distance_km: f64,
    pub income: i64,
    pub delivery_time_min: u32,
    pub game_time_min: Option<u32>,
    pub remaining_time_min: Option<i64>,
    pub last_seen_at_utc: String,
    pub status: String,
    pub cargo_damage: f64,
    pub job_market: String,
    pub special_job: bool,
    pub ingame_income: Option<i64>,
    pub vtc_expected_income: Option<i64>,
    pub result_status: Option<String>,
    pub planned_distance_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobStats {
    pub total_jobs: i64,
    pub total_income: i64,
    pub average_distance_km: f64,
    pub success_rate: f64,
    pub completed_jobs: i64,
    pub failed_jobs: i64,
    pub cancelled_jobs: i64,
    pub abandoned_jobs: i64,
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS career_job_log (
            job_id TEXT PRIMARY KEY,
            started_at_utc TEXT NOT NULL,
            ended_at_utc TEXT,
            origin_city TEXT NOT NULL,
            destination_city TEXT NOT NULL,
            source_company TEXT NOT NULL,
            destination_company TEXT NOT NULL,
            cargo TEXT NOT NULL,
            planned_distance_km REAL NOT NULL DEFAULT 0,
            income INTEGER NOT NULL DEFAULT 0,
            delivery_time_min INTEGER NOT NULL DEFAULT 0,
            last_seen_at_utc TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            cargo_damage REAL NOT NULL DEFAULT 0,
            job_market TEXT NOT NULL DEFAULT '',
            special_job INTEGER NOT NULL DEFAULT 0
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn upsert_active_job(conn: &Connection, entry: &JobLogEntry) -> Result<(), String> {
    // Only set started_at_utc when inserting a new job_id. Updates should not reset it.
    conn.execute(
        r#"
        INSERT INTO career_job_log (
            job_id,
            started_at_utc,
            ended_at_utc,
            origin_city,
            destination_city,
            source_company,
            destination_company,
            cargo,
            planned_distance_km,
            income,
            delivery_time_min,
            last_seen_at_utc,
            status,
            cargo_damage,
            job_market,
            special_job
        )
        VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
        )
        ON CONFLICT(job_id) DO UPDATE SET
            ended_at_utc = excluded.ended_at_utc,
            origin_city = excluded.origin_city,
            destination_city = excluded.destination_city,
            source_company = excluded.source_company,
            destination_company = excluded.destination_company,
            cargo = excluded.cargo,
            planned_distance_km = excluded.planned_distance_km,
            income = excluded.income,
            delivery_time_min = excluded.delivery_time_min,
            last_seen_at_utc = excluded.last_seen_at_utc,
            status = excluded.status,
            cargo_damage = excluded.cargo_damage,
            job_market = excluded.job_market,
            special_job = excluded.special_job
        "#,
        params![
            entry.job_id,
            entry.started_at_utc,
            entry.ended_at_utc,
            entry.origin_city,
            entry.destination_city,
            entry.source_company,
            entry.destination_company,
            entry.cargo,
            entry.planned_distance_km,
            entry.income,
            entry.delivery_time_min as i64,
            entry.last_seen_at_utc,
            entry.status,
            entry.cargo_damage,
            entry.job_market,
            if entry.special_job { 1 } else { 0 }
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn mark_job_finished(
    conn: &Connection,
    job_id: &str,
    ended_at_utc: &str,
    status: &str,
    cargo_damage: f64,
) -> Result<(), String> {
    conn.execute(
        r#"
        UPDATE career_job_log
        SET
            ended_at_utc = ?2,
            status = ?3,
            cargo_damage = ?4
        WHERE job_id = ?1
        "#,
        params![job_id, ended_at_utc, status, cargo_damage],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn get_job(conn: &Connection, job_id: &str) -> Result<Option<JobLogEntry>, String> {
    conn.query_row(
        r#"
        SELECT
            job_id,
            started_at_utc,
            ended_at_utc,
            origin_city,
            destination_city,
            source_company,
            destination_company,
            cargo,
            planned_distance_km,
            income,
            delivery_time_min,
            last_seen_at_utc,
            status,
            cargo_damage,
            job_market,
            special_job
        FROM career_job_log
        WHERE job_id = ?1
        "#,
        [job_id],
        |row| {
            Ok(JobLogEntry {
                job_id: row.get(0)?,
                started_at_utc: row.get(1)?,
                ended_at_utc: row.get(2)?,
                origin_city: row.get(3)?,
                destination_city: row.get(4)?,
                source_company: row.get(5)?,
                destination_company: row.get(6)?,
                cargo: row.get(7)?,
                planned_distance_km: row.get(8)?,
                income: row.get(9)?,
                delivery_time_min: row.get::<_, i64>(10)? as u32,
                game_time_min: None,
                remaining_time_min: None,
                last_seen_at_utc: row.get(11)?,
                status: row.get(12)?,
                cargo_damage: row.get(13)?,
                job_market: row.get(14)?,
                special_job: row.get::<_, i64>(15)? != 0,
                ingame_income: None,
                vtc_expected_income: None,
                result_status: None,
                planned_distance_source: None,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn list_recent_jobs(conn: &Connection, limit: usize) -> Result<Vec<JobLogEntry>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                job_id,
                started_at_utc,
                ended_at_utc,
                origin_city,
                destination_city,
                source_company,
                destination_company,
                cargo,
                planned_distance_km,
                income,
                delivery_time_min,
                last_seen_at_utc,
                status,
                cargo_damage,
                job_market,
                special_job
            FROM career_job_log
            ORDER BY started_at_utc DESC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(JobLogEntry {
                job_id: row.get(0)?,
                started_at_utc: row.get(1)?,
                ended_at_utc: row.get(2)?,
                origin_city: row.get(3)?,
                destination_city: row.get(4)?,
                source_company: row.get(5)?,
                destination_company: row.get(6)?,
                cargo: row.get(7)?,
                planned_distance_km: row.get(8)?,
                income: row.get(9)?,
                delivery_time_min: row.get::<_, i64>(10)? as u32,
                game_time_min: None,
                remaining_time_min: None,
                last_seen_at_utc: row.get(11)?,
                status: row.get(12)?,
                cargo_damage: row.get(13)?,
                job_market: row.get(14)?,
                special_job: row.get::<_, i64>(15)? != 0,
                ingame_income: None,
                vtc_expected_income: None,
                result_status: None,
                planned_distance_source: None,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn load_job_stats(conn: &Connection) -> Result<JobStats, String> {
    let mut jobs = list_recent_jobs(conn, 5000)?;
    enrich_job_entries(conn, &mut jobs)?;

    let mut total_income = 0_i64;
    let mut total_distance = 0.0_f64;
    let mut total_distance_count = 0_i64;
    let mut completed_jobs = 0_i64;
    let mut failed_jobs = 0_i64;
    let mut cancelled_jobs = 0_i64;
    let mut abandoned_jobs = 0_i64;

    for job in &jobs {
        if job.planned_distance_km > 0.0 {
            total_distance += job.planned_distance_km;
            total_distance_count += 1;
        }

        match job.status.as_str() {
            "completed" => {
                completed_jobs += 1;
                total_income += job
                    .vtc_expected_income
                    .or(job.ingame_income)
                    .unwrap_or(job.income);
            }
            "failed" => failed_jobs += 1,
            "cancelled" => cancelled_jobs += 1,
            "abandoned" => abandoned_jobs += 1,
            _ => {}
        }
    }

    let terminal_jobs = completed_jobs + failed_jobs + cancelled_jobs + abandoned_jobs;
    Ok(JobStats {
        total_jobs: jobs.len() as i64,
        total_income,
        average_distance_km: if total_distance_count > 0 {
            total_distance / total_distance_count as f64
        } else {
            0.0
        },
        success_rate: if terminal_jobs > 0 {
            completed_jobs as f64 / terminal_jobs as f64
        } else {
            0.0
        },
        completed_jobs,
        failed_jobs,
        cancelled_jobs,
        abandoned_jobs,
    })
}

pub fn enrich_job_entries(conn: &Connection, entries: &mut [JobLogEntry]) -> Result<(), String> {
    for entry in entries {
        enrich_job_entry(conn, entry)?;
    }
    Ok(())
}

pub fn enrich_job_entry(conn: &Connection, entry: &mut JobLogEntry) -> Result<(), String> {
    entry.ingame_income = Some(entry.income);
    entry.result_status = Some(entry.status.clone());

    let (planned_distance_km, planned_distance_source) =
        resolve_planned_distance(conn, entry)?.unwrap_or((
            entry.planned_distance_km.max(0.0),
            if entry.planned_distance_km > 0.0 {
                "telemetry".to_string()
            } else {
                "missing".to_string()
            },
        ));
    entry.planned_distance_km = planned_distance_km;
    entry.planned_distance_source = Some(planned_distance_source);
    entry.vtc_expected_income = resolve_vtc_expected_income(conn, entry)?;

    Ok(())
}

fn resolve_planned_distance(
    conn: &Connection,
    entry: &JobLogEntry,
) -> Result<Option<(f64, String)>, String> {
    if entry.planned_distance_km > 0.0 {
        return Ok(Some((entry.planned_distance_km, "telemetry".to_string())));
    }

    if let Some(distance_km) = conn
        .query_row(
            "SELECT distance_km FROM dispatcher_jobs WHERE id = ?1 AND distance_km > 0 ORDER BY updated_at_utc DESC LIMIT 1",
            [entry.job_id.as_str()],
            |row| row.get::<_, f64>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        return Ok(Some((distance_km, "dispatcher".to_string())));
    }

    if let Some(distance_km) = conn
        .query_row(
            "SELECT distance_km FROM trips WHERE job_id = ?1 AND distance_km > 0 ORDER BY id DESC LIMIT 1",
            [entry.job_id.as_str()],
            |row| row.get::<_, f64>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        return Ok(Some((distance_km, "trip".to_string())));
    }

    let estimated = estimate_distance_from_ingame_reward(entry);
    Ok(estimated.map(|value| (value, "reward_estimate".to_string())))
}

fn resolve_vtc_expected_income(
    conn: &Connection,
    entry: &JobLogEntry,
) -> Result<Option<i64>, String> {
    if let Some(total_reward) = conn
        .query_row(
            "SELECT total_reward FROM dispatcher_jobs WHERE id = ?1 ORDER BY updated_at_utc DESC LIMIT 1",
            [entry.job_id.as_str()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        return Ok(Some(total_reward));
    }

    let distance_km = entry.planned_distance_km.max(0.0);
    if distance_km <= 0.0 {
        return Ok(None);
    }

    let company_name = normalize_text(&entry.source_company)
        .or_else(|| normalize_text(&entry.destination_company))
        .unwrap_or_else(|| "Open Market".to_string());
    let origin_country_code = infer_country_code(&entry.origin_city);
    let destination_country_code = infer_country_code(&entry.destination_city);
    let compensation = compensation_service::calculate_job_compensation(
        conn,
        &JobCompensationInput {
            company_id: company_key_from_name(&company_name),
            company_name: Some(company_name),
            distance_km,
            base_rate_type: base_rate_type_for_job(entry),
            equipment_type: equipment_type_for_job(entry),
            cargo_type: infer_cargo_type(&entry.cargo),
            urgency: infer_urgency(entry),
            origin_country_code,
            destination_country_code,
            market_seed: fnv1a64(&entry.job_id),
        },
    )?;

    Ok(Some(compensation.final_price))
}

fn estimate_distance_from_ingame_reward(entry: &JobLogEntry) -> Option<f64> {
    let income = entry.income.max(0) as f64;
    if income <= 0.0 {
        return None;
    }

    let estimated_rate = nominal_ingame_rate_per_km(entry);
    if estimated_rate <= 0.0 {
        return None;
    }

    Some(((income / estimated_rate) * 10.0).round() / 10.0)
}

fn nominal_ingame_rate_per_km(entry: &JobLogEntry) -> f64 {
    let market_rate: f64 = match entry.job_market.trim().to_ascii_lowercase().as_str() {
        "quick_job" => 1.45_f64,
        "freight_market" => 1.95_f64,
        "cargo_market" => 2.25_f64,
        "company_contract" => 2.35_f64,
        "premium_special" => 2.65_f64,
        _ if entry.special_job => 2.55_f64,
        _ => 1.85_f64,
    };

    let cargo_modifier: f64 = match infer_cargo_type(&entry.cargo) {
        CargoType::Standard => 1.00_f64,
        CargoType::Fragile => 1.05_f64,
        CargoType::Refrigerated => 1.06_f64,
        CargoType::Valuable => 1.10_f64,
        CargoType::Hazardous => 1.12_f64,
        CargoType::Oversize => 1.15_f64,
    };

    (market_rate * cargo_modifier).max(0.5)
}

fn base_rate_type_for_job(entry: &JobLogEntry) -> BaseRateType {
    match entry.job_market.trim().to_ascii_lowercase().as_str() {
        "quick_job" => BaseRateType::QuickJob,
        "cargo_market" | "premium_special" => BaseRateType::OwnTruckOwnTrailer,
        _ => BaseRateType::OwnTruck,
    }
}

fn equipment_type_for_job(entry: &JobLogEntry) -> EquipmentType {
    match entry.job_market.trim().to_ascii_lowercase().as_str() {
        "quick_job" => EquipmentType::QuickJob,
        "cargo_market" | "premium_special" => EquipmentType::OwnTruckOwnTrailer,
        _ => EquipmentType::OwnTruck,
    }
}

fn infer_cargo_type(cargo: &str) -> CargoType {
    let value = cargo.trim().to_ascii_lowercase();
    if value.contains("chemical") || value.contains("hazard") {
        CargoType::Hazardous
    } else if value.contains("medical")
        || value.contains("fragile")
        || value.contains("glass")
    {
        CargoType::Fragile
    } else if value.contains("fresh")
        || value.contains("refrigerated")
        || value.contains("cold")
        || value.contains("food")
    {
        CargoType::Refrigerated
    } else if value.contains("valuable") || value.contains("electronics") {
        CargoType::Valuable
    } else if value.contains("oversize")
        || value.contains("heavy")
        || value.contains("bagger")
        || value.contains("excavator")
    {
        CargoType::Oversize
    } else {
        CargoType::Standard
    }
}

fn infer_urgency(entry: &JobLogEntry) -> Urgency {
    match entry.job_market.trim().to_ascii_lowercase().as_str() {
        "premium_special" => Urgency::Express,
        "company_contract" => Urgency::Priority,
        _ if entry.special_job => Urgency::Priority,
        _ => Urgency::Normal,
    }
}

fn infer_country_code(city: &str) -> String {
    compensation_service::infer_country_code_from_city(city)
        .unwrap_or("DE")
        .to_string()
}

fn normalize_text(value: &str) -> Option<String> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized == "-" {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn company_key_from_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }

    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "company-unknown".to_string()
    } else {
        format!("company-{trimmed}")
    }
}

fn fnv1a64(text: &str) -> u64 {
    let mut hash: u64 = 14695981039346656037;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}
