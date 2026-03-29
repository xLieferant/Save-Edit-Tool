use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub fn default_db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        })
        .join("SimNexusHub")
        .join("logbook.sqlite")
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
            started_at_utc TEXT NOT NULL,
            ended_at_utc TEXT,
            origin TEXT,
            destination TEXT,
            distance_km REAL,
            income INTEGER,
            raw_telemetry_json TEXT
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
