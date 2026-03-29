use rusqlite::Connection;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::command;
use tauri::State;

use crate::features::hub::events::CareerStatus;
use crate::state::CareerState;

#[derive(Debug, Clone, Serialize)]
pub struct TripSummary {
    pub id: i64,
    pub started_at_utc: String,
    pub ended_at_utc: Option<String>,
    pub origin: Option<String>,
    pub destination: Option<String>,
    pub distance_km: Option<f64>,
    pub income: Option<i64>,
}

#[command]
pub fn career_get_status(career: State<'_, CareerState>) -> Result<CareerStatus, String> {
    let runtime = career.runtime.as_ref();
    Ok(CareerStatus {
        ets2_running: runtime.ets2_running.load(Ordering::Relaxed),
        ats_running: runtime.ats_running.load(Ordering::Relaxed),
        telemetry_running: runtime.telemetry_running.load(Ordering::Relaxed),
    })
}

#[command]
pub fn career_list_trips(career: State<'_, CareerState>) -> Result<Vec<TripSummary>, String> {
    let runtime = career.runtime.as_ref();
    let db_path = runtime
        .db_path
        .lock()
        .map_err(|_| "Career db_path lock poisoned".to_string())?
        .clone()
        .ok_or_else(|| "Career database path not initialized".to_string())?;

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, started_at_utc, ended_at_utc, origin, destination, distance_km, income
            FROM trips
            ORDER BY id DESC
            LIMIT 200
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(TripSummary {
                id: row.get(0)?,
                started_at_utc: row.get(1)?,
                ended_at_utc: row.get(2)?,
                origin: row.get(3)?,
                destination: row.get(4)?,
                distance_km: row.get(5)?,
                income: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }

    Ok(out)
}
