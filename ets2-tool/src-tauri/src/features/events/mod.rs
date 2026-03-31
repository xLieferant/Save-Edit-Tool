use chrono::Utc;
use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerEvent {
    pub event_id: String,
    pub category: String,
    pub title: String,
    pub impact: String,
    pub severity: String,
    pub occurred_at_utc: String,
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS career_events (
            event_id TEXT PRIMARY KEY,
            category TEXT NOT NULL,
            title TEXT NOT NULL,
            impact TEXT NOT NULL,
            severity TEXT NOT NULL,
            occurred_at_utc TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    let seeded = [
        (
            "seed-dispatch-01",
            "dispatcher",
            "Priority corridor opened",
            "Berlin to Prague medical line is ready for dispatch.",
            "high",
        ),
        (
            "seed-bank-01",
            "bank",
            "Debt schedule active",
            "Automatic installment plan is running on each completed trip.",
            "medium",
        ),
        (
            "seed-fleet-01",
            "fleet",
            "Maintenance tracker online",
            "Player truck wear is now tracked against active route distance.",
            "low",
        ),
    ];

    for (event_id, category, title, impact, severity) in seeded {
        conn.execute(
            r#"
            INSERT OR IGNORE INTO career_events (
                event_id,
                category,
                title,
                impact,
                severity,
                occurred_at_utc
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![event_id, category, title, impact, severity, Utc::now().to_rfc3339()],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn list_recent_events(conn: &Connection, limit: usize) -> Result<Vec<CareerEvent>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT event_id, category, title, impact, severity, occurred_at_utc
            FROM career_events
            ORDER BY occurred_at_utc DESC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(CareerEvent {
                event_id: row.get(0)?,
                category: row.get(1)?,
                title: row.get(2)?,
                impact: row.get(3)?,
                severity: row.get(4)?,
                occurred_at_utc: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn record_event(
    conn: &Connection,
    category: &str,
    title: &str,
    impact: &str,
    severity: &str,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO career_events (
            event_id,
            category,
            title,
            impact,
            severity,
            occurred_at_utc
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        params![
            format!(
                "evt-{}-{}",
                Utc::now().timestamp_nanos_opt().unwrap_or_else(|| Utc::now().timestamp_micros() * 1000),
                severity
            ),
            category,
            title,
            impact,
            severity,
            Utc::now().to_rfc3339()
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
