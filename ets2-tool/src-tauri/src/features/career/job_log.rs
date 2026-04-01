use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

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
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobStats {
    pub total_jobs: i64,
    pub total_income: i64,
    pub average_distance_km: f64,
    pub success_rate: f64,
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
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn load_job_stats(conn: &Connection) -> Result<JobStats, String> {
    conn.query_row(
        r#"
        SELECT
            COUNT(*),
            COALESCE(SUM(income), 0),
            COALESCE(AVG(NULLIF(planned_distance_km, 0)), 0),
            COALESCE(AVG(CASE WHEN status = 'completed' THEN 1.0 ELSE 0.0 END), 0)
        FROM career_job_log
        "#,
        [],
        |row| {
            Ok(JobStats {
                total_jobs: row.get(0)?,
                total_income: row.get(1)?,
                average_distance_km: row.get(2)?,
                success_rate: row.get(3)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}
