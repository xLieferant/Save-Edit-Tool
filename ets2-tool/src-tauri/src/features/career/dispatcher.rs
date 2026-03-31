use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub source: String,
    pub destination: String,
    pub distance_km: f64,
    pub price_per_km: f64,
    pub cargo: String,
    pub accepted: bool,
    pub completed: bool,
    pub progress_km: f64,
    pub estimated_payout: i64,
    pub remaining_km: f64,
}

#[derive(Debug, Clone)]
pub struct JobAssignment {
    pub id: String,
    pub source: String,
    pub destination: String,
    pub distance_km: f64,
    pub price_per_km: f64,
    pub cargo: String,
    pub progress_km: f64,
}

const JOB_TEMPLATES: [(&str, &str, f64, f64, &str); 8] = [
    ("Hamburg", "Prague", 642.0, 33.5, "Industrial components"),
    ("Berlin", "Vienna", 684.0, 31.0, "Medical cargo"),
    ("Warsaw", "Brno", 518.0, 28.5, "Dry food pallets"),
    ("Munich", "Genoa", 734.0, 36.0, "Machine parts"),
    ("Dresden", "Rotterdam", 812.0, 34.5, "Chemical containers"),
    ("Leipzig", "Oslo", 1284.0, 39.0, "Special cargo"),
    ("Frankfurt", "Lyon", 711.0, 30.0, "Retail freight"),
    ("Kiel", "Brussels", 596.0, 29.5, "Packaged goods"),
];

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS career_jobs (
            id TEXT PRIMARY KEY,
            source TEXT NOT NULL,
            destination TEXT NOT NULL,
            distance_km REAL NOT NULL,
            price_per_km REAL NOT NULL,
            cargo TEXT NOT NULL,
            accepted INTEGER NOT NULL DEFAULT 0,
            completed INTEGER NOT NULL DEFAULT 0,
            progress_km REAL NOT NULL DEFAULT 0,
            created_at_utc TEXT NOT NULL,
            accepted_at_utc TEXT,
            completed_at_utc TEXT
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    let open_jobs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_jobs WHERE completed = 0",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if open_jobs == 0 {
        let _ = generate_jobs(conn)?;
    }

    Ok(())
}

pub fn generate_jobs(conn: &Connection) -> Result<Vec<Job>, String> {
    let open_jobs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM career_jobs WHERE completed = 0",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let needed = (6_i64 - open_jobs).max(0) as usize;
    let seed = Utc::now().timestamp_millis().unsigned_abs() as usize;

    for index in 0..needed {
        let template = JOB_TEMPLATES[(seed + index) % JOB_TEMPLATES.len()];
        let job_id = format!("dispatch-{}-{}", Utc::now().timestamp_millis(), index);
        conn.execute(
            r#"
            INSERT INTO career_jobs (
                id,
                source,
                destination,
                distance_km,
                price_per_km,
                cargo,
                accepted,
                completed,
                progress_km,
                created_at_utc
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 0, ?7)
            "#,
            params![
                job_id,
                template.0,
                template.1,
                template.2,
                template.3,
                template.4,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    list_jobs(conn, 12)
}

pub fn list_jobs(conn: &Connection, limit: usize) -> Result<Vec<Job>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                source,
                destination,
                distance_km,
                price_per_km,
                cargo,
                accepted,
                completed,
                progress_km
            FROM career_jobs
            WHERE completed = 0
            ORDER BY accepted DESC, created_at_utc DESC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            let distance_km: f64 = row.get(3)?;
            let price_per_km: f64 = row.get(4)?;
            let progress_km: f64 = row.get(8)?;
            Ok(Job {
                id: row.get(0)?,
                source: row.get(1)?,
                destination: row.get(2)?,
                distance_km,
                price_per_km,
                cargo: row.get(5)?,
                accepted: row.get::<_, i64>(6)? != 0,
                completed: row.get::<_, i64>(7)? != 0,
                progress_km,
                estimated_payout: (distance_km * price_per_km).round() as i64,
                remaining_km: (distance_km - progress_km).max(0.0),
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn current_job(conn: &Connection) -> Result<Option<Job>, String> {
    let mut jobs = list_jobs(conn, 12)?;
    Ok(jobs.drain(..).find(|job| job.accepted && !job.completed))
}

pub fn current_assignment(conn: &Connection) -> Result<Option<JobAssignment>, String> {
    Ok(current_job(conn)?.map(|job| JobAssignment {
        id: job.id,
        source: job.source,
        destination: job.destination,
        distance_km: job.distance_km,
        price_per_km: job.price_per_km,
        cargo: job.cargo,
        progress_km: job.progress_km,
    }))
}

pub fn find_job_by_id(conn: &Connection, job_id: &str) -> Result<Option<Job>, String> {
    conn.query_row(
        r#"
        SELECT
            id,
            source,
            destination,
            distance_km,
            price_per_km,
            cargo,
            accepted,
            completed,
            progress_km
        FROM career_jobs
        WHERE id = ?1
        "#,
        [job_id],
        |row| {
            let distance_km: f64 = row.get(3)?;
            let price_per_km: f64 = row.get(4)?;
            let progress_km: f64 = row.get(8)?;
            Ok(Job {
                id: row.get(0)?,
                source: row.get(1)?,
                destination: row.get(2)?,
                distance_km,
                price_per_km,
                cargo: row.get(5)?,
                accepted: row.get::<_, i64>(6)? != 0,
                completed: row.get::<_, i64>(7)? != 0,
                progress_km,
                estimated_payout: (distance_km * price_per_km).round() as i64,
                remaining_km: (distance_km - progress_km).max(0.0),
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn accept_job(conn: &Connection, job_id: &str) -> Result<Job, String> {
    conn.execute(
        "UPDATE career_jobs SET accepted = 0 WHERE completed = 0",
        [],
    )
    .map_err(|e| e.to_string())?;

    let changed = conn
        .execute(
            r#"
            UPDATE career_jobs
            SET accepted = 1, accepted_at_utc = ?2
            WHERE id = ?1 AND completed = 0
            "#,
            params![job_id, Utc::now().to_rfc3339()],
        )
        .map_err(|e| e.to_string())?;

    if changed == 0 {
        return Err(format!("Job not found: {job_id}"));
    }

    find_job_by_id(conn, job_id)?
        .ok_or_else(|| format!("Job not found after accept: {job_id}"))
}

pub fn store_progress(conn: &Connection, job_id: &str, progress_km: f64) -> Result<(), String> {
    conn.execute(
        r#"
        UPDATE career_jobs
        SET progress_km = CASE
            WHEN progress_km > ?2 THEN progress_km
            ELSE ?2
        END
        WHERE id = ?1 AND completed = 0
        "#,
        params![job_id, progress_km.max(0.0)],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn complete_job(conn: &Connection, job_id: &str) -> Result<Job, String> {
    let job = find_job_by_id(conn, job_id)?.ok_or_else(|| format!("Job not found: {job_id}"))?;
    conn.execute(
        r#"
        UPDATE career_jobs
        SET
            accepted = 0,
            completed = 1,
            progress_km = ?2,
            completed_at_utc = ?3
        WHERE id = ?1
        "#,
        params![job_id, job.distance_km, Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;

    Ok(Job {
        progress_km: job.distance_km,
        completed: true,
        accepted: false,
        remaining_km: 0.0,
        ..job
    })
}
