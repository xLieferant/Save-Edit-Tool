use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReputationState {
    pub score: i64,
    pub xp_points: i64,
    pub level: i64,
    pub label: String,
    pub completed_jobs: i64,
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS reputation_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            score INTEGER NOT NULL,
            xp_points INTEGER NOT NULL,
            completed_jobs INTEGER NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        r#"
        INSERT OR IGNORE INTO reputation_state (id, score, xp_points, completed_jobs)
        VALUES (1, 42, 0, 0)
        "#,
        [],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn load_state(conn: &Connection) -> Result<ReputationState, String> {
    conn.query_row(
        r#"
        SELECT score, xp_points, completed_jobs
        FROM reputation_state
        WHERE id = 1
        "#,
        [],
        |row| {
            let score: i64 = row.get(0)?;
            let xp_points: i64 = row.get(1)?;
            let completed_jobs: i64 = row.get(2)?;
            Ok(ReputationState {
                score,
                xp_points,
                level: level_from_xp(xp_points),
                label: label_from_score(score).to_string(),
                completed_jobs,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn apply_trip_outcome(
    conn: &Connection,
    distance_km: f64,
    speeding_events: i64,
) -> Result<ReputationState, String> {
    let current = load_state(conn)?;
    let distance_bonus = if distance_km >= 200.0 { 2 } else { 0 };
    let score_delta = (4 + distance_bonus - speeding_events * 2).max(-12);
    let xp_gain = ((distance_km * 3.5).round() as i64 - speeding_events * 15).max(40);
    let next_score = (current.score + score_delta).max(0);
    let next_xp = (current.xp_points + xp_gain).max(0);
    let next_completed = current.completed_jobs + 1;

    conn.execute(
        r#"
        UPDATE reputation_state
        SET score = ?1, xp_points = ?2, completed_jobs = ?3
        WHERE id = 1
        "#,
        params![next_score, next_xp, next_completed],
    )
    .map_err(|e| e.to_string())?;

    load_state(conn)
}

fn level_from_xp(xp_points: i64) -> i64 {
    (xp_points / 750) + 1
}

fn label_from_score(score: i64) -> &'static str {
    match score {
        value if value >= 120 => "Legendary",
        value if value >= 90 => "Elite",
        value if value >= 60 => "Trusted",
        value if value >= 30 => "Established",
        _ => "Starting",
    }
}
