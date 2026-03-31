use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FleetAssetSummary {
    pub asset_id: String,
    pub kind: String,
    pub brand: String,
    pub model: String,
    pub condition_percent: f64,
    pub insurance_tier: String,
    pub status: String,
    pub leased: bool,
    pub service_due_km: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FleetOverview {
    pub total_assets: i64,
    pub trucks: i64,
    pub trailers: i64,
    pub avg_condition: f64,
    pub player_condition: f64,
    pub maintenance_risk: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct WearOutcome {
    pub repair_reserve: i64,
    pub player_condition: f64,
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS fleet_assets (
            asset_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            brand TEXT NOT NULL,
            model TEXT NOT NULL,
            condition_percent REAL NOT NULL,
            insurance_tier TEXT NOT NULL,
            status TEXT NOT NULL,
            leased INTEGER NOT NULL DEFAULT 0,
            service_due_km REAL NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    let assets = [
        (
            "asset-player-truck",
            "truck",
            "Scania",
            "S 730",
            94.0_f64,
            "Premium",
            "player",
            0_i64,
            14000.0_f64,
        ),
        (
            "asset-fleet-truck-01",
            "truck",
            "Volvo",
            "FH16",
            88.0_f64,
            "Plus",
            "assigned",
            1_i64,
            9600.0_f64,
        ),
        (
            "asset-trailer-01",
            "trailer",
            "Krone",
            "Cool Liner",
            91.0_f64,
            "Basic",
            "ready",
            0_i64,
            18500.0_f64,
        ),
    ];

    for (asset_id, kind, brand, model, condition_percent, insurance_tier, status, leased, service_due_km) in assets {
        conn.execute(
            r#"
            INSERT OR IGNORE INTO fleet_assets (
                asset_id,
                kind,
                brand,
                model,
                condition_percent,
                insurance_tier,
                status,
                leased,
                service_due_km
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                asset_id,
                kind,
                brand,
                model,
                condition_percent,
                insurance_tier,
                status,
                leased,
                service_due_km
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn load_assets(conn: &Connection, limit: usize) -> Result<Vec<FleetAssetSummary>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT asset_id, kind, brand, model, condition_percent, insurance_tier, status, leased, service_due_km
            FROM fleet_assets
            ORDER BY
                CASE status
                    WHEN 'player' THEN 0
                    WHEN 'assigned' THEN 1
                    ELSE 2
                END,
                kind ASC,
                brand ASC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(FleetAssetSummary {
                asset_id: row.get(0)?,
                kind: row.get(1)?,
                brand: row.get(2)?,
                model: row.get(3)?,
                condition_percent: row.get(4)?,
                insurance_tier: row.get(5)?,
                status: row.get(6)?,
                leased: row.get::<_, i64>(7)? != 0,
                service_due_km: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn load_overview(conn: &Connection) -> Result<FleetOverview, String> {
    conn.query_row(
        r#"
        SELECT
            COUNT(*) AS total_assets,
            SUM(CASE WHEN kind = 'truck' THEN 1 ELSE 0 END) AS trucks,
            SUM(CASE WHEN kind = 'trailer' THEN 1 ELSE 0 END) AS trailers,
            AVG(condition_percent) AS avg_condition,
            MAX(CASE WHEN status = 'player' THEN condition_percent ELSE NULL END) AS player_condition,
            MIN(service_due_km) AS next_service_due
        FROM fleet_assets
        "#,
        [],
        |row| {
            let next_service_due = row.get::<_, Option<f64>>(5)?.unwrap_or(99999.0);
            Ok(FleetOverview {
                total_assets: row.get(0)?,
                trucks: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                trailers: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                avg_condition: row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                player_condition: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                maintenance_risk: next_service_due <= 2000.0,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn apply_trip_wear(
    conn: &Connection,
    distance_km: f64,
    speeding_events: i64,
) -> Result<WearOutcome, String> {
    let (current_condition, current_due): (f64, f64) = conn
        .query_row(
            r#"
            SELECT condition_percent, service_due_km
            FROM fleet_assets
            WHERE asset_id = 'asset-player-truck'
            "#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let wear = (distance_km * 0.0085) + (speeding_events as f64 * 0.55);
    let next_condition = (current_condition - wear).clamp(45.0, 100.0);
    let next_due = (current_due - distance_km).max(0.0);
    let repair_reserve = if next_condition < 82.0 {
        ((82.0 - next_condition) * 22.0).round() as i64
    } else {
        0
    };

    conn.execute(
        r#"
        UPDATE fleet_assets
        SET condition_percent = ?1, service_due_km = ?2
        WHERE asset_id = 'asset-player-truck'
        "#,
        params![next_condition, next_due],
    )
    .map_err(|e| e.to_string())?;

    Ok(WearOutcome {
        repair_reserve,
        player_condition: next_condition,
    })
}
