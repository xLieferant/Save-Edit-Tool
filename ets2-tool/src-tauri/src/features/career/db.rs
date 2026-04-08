use rusqlite::Connection;
use std::path::{Path, PathBuf};

use crate::features::career::dispatcher;
use crate::features::career::job_log;
use crate::features::{auth, companies, vtc};
use crate::features::{bank, contracts, economy, employees, events, fleet, reputation};
use crate::shared::sqlite_schema::ensure_columns;

pub fn default_db_path() -> PathBuf {
    crate::db::sqlite::app_db_path()
}

pub fn init_logbook(db_path: &Path) -> Result<(), String> {
    if let Some(dir) = db_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS trips (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            job_id TEXT,
            contract_id TEXT,
            started_at_utc TEXT NOT NULL,
            ended_at_utc TEXT,
            origin TEXT,
            destination TEXT,
            cargo TEXT,
            distance_km REAL,
            income INTEGER,
            damage REAL NOT NULL DEFAULT 0,
            duration_seconds INTEGER NOT NULL DEFAULT 0,
            avg_speed_kph REAL NOT NULL DEFAULT 0,
            max_speed_kph REAL NOT NULL DEFAULT 0,
            speeding_events INTEGER NOT NULL DEFAULT 0,
            fuel_used_liters REAL NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'active',
            raw_telemetry_json TEXT
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    ensure_trip_columns(&conn)?;
    conn.execute(
        r#"
        UPDATE trips
        SET
            status = CASE WHEN status = 'active' THEN 'aborted' ELSE status END,
            ended_at_utc = COALESCE(ended_at_utc, started_at_utc)
        WHERE status = 'active' AND ended_at_utc IS NULL
        "#,
        [],
    )
    .map_err(|e| e.to_string())?;

    economy::ensure_tables(&conn)?;
    reputation::ensure_tables(&conn)?;
    contracts::ensure_tables(&conn)?;
    bank::ensure_tables(&conn)?;
    events::ensure_tables(&conn)?;
    employees::ensure_tables(&conn)?;
    fleet::ensure_tables(&conn)?;
    dispatcher::ensure_tables(&conn)?;
    job_log::ensure_tables(&conn)?;

    auth::db::ensure_tables(&conn)?;
    companies::db::ensure_tables(&conn)?;
    vtc::db::ensure_tables(&conn)?;
    auth::service::seed_default_admin(&conn)?;

    Ok(())
}

fn ensure_trip_columns(conn: &Connection) -> Result<(), String> {
    let required = [
        ("job_id", "TEXT"),
        ("contract_id", "TEXT"),
        ("cargo", "TEXT"),
        ("damage", "REAL NOT NULL DEFAULT 0"),
        ("duration_seconds", "INTEGER NOT NULL DEFAULT 0"),
        ("avg_speed_kph", "REAL NOT NULL DEFAULT 0"),
        ("max_speed_kph", "REAL NOT NULL DEFAULT 0"),
        ("speeding_events", "INTEGER NOT NULL DEFAULT 0"),
        ("fuel_used_liters", "REAL NOT NULL DEFAULT 0"),
        ("status", "TEXT NOT NULL DEFAULT 'active'"),
    ];
    ensure_columns(conn, "trips", &required)?;
    Ok(())
}
