use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EconomyState {
    pub company_name: String,
    pub price_per_km: f64,
    pub diesel_price_per_liter: f64,
    pub toll_per_km: f64,
    pub insurance_daily_cost: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FreightOffer {
    pub offer_id: String,
    pub origin: String,
    pub destination: String,
    pub cargo: String,
    pub payout: i64,
    pub eta_hours: i64,
    pub risk: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TripCostBreakdown {
    pub fuel_cost: i64,
    pub toll_cost: i64,
    pub insurance_cost: i64,
    pub total_cost: i64,
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS economy_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            company_name TEXT NOT NULL,
            price_per_km REAL NOT NULL,
            diesel_price_per_liter REAL NOT NULL,
            toll_per_km REAL NOT NULL,
            insurance_daily_cost INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS freight_market_offers (
            offer_id TEXT PRIMARY KEY,
            origin TEXT NOT NULL,
            destination TEXT NOT NULL,
            cargo TEXT NOT NULL,
            payout INTEGER NOT NULL,
            eta_hours INTEGER NOT NULL,
            risk TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        r#"
        INSERT OR IGNORE INTO economy_state (
            id,
            company_name,
            price_per_km,
            diesel_price_per_liter,
            toll_per_km,
            insurance_daily_cost
        )
        VALUES (1, 'SimNexus Logistics', 24.5, 1.78, 0.19, 360)
        "#,
        [],
    )
    .map_err(|e| e.to_string())?;

    let offers = [
        ("offer-med-ber-prg", "Berlin", "Prague", "Medical supplies", 18200_i64, 6_i64, "medium"),
        ("offer-steel-ham-lyo", "Hamburg", "Lyon", "Industrial steel", 31980_i64, 12_i64, "high"),
        ("offer-food-waw-vie", "Warsaw", "Vienna", "Fresh produce", 22460_i64, 9_i64, "low"),
    ];

    for (offer_id, origin, destination, cargo, payout, eta_hours, risk) in offers {
        conn.execute(
            r#"
            INSERT OR IGNORE INTO freight_market_offers (
                offer_id,
                origin,
                destination,
                cargo,
                payout,
                eta_hours,
                risk
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![offer_id, origin, destination, cargo, payout, eta_hours, risk],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn load_state(conn: &Connection) -> Result<EconomyState, String> {
    conn.query_row(
        r#"
        SELECT company_name, price_per_km, diesel_price_per_liter, toll_per_km, insurance_daily_cost
        FROM economy_state
        WHERE id = 1
        "#,
        [],
        |row| {
            Ok(EconomyState {
                company_name: row.get(0)?,
                price_per_km: row.get(1)?,
                diesel_price_per_liter: row.get(2)?,
                toll_per_km: row.get(3)?,
                insurance_daily_cost: row.get(4)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn list_freight_offers(conn: &Connection, limit: usize) -> Result<Vec<FreightOffer>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT offer_id, origin, destination, cargo, payout, eta_hours, risk
            FROM freight_market_offers
            ORDER BY payout DESC, eta_hours ASC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(FreightOffer {
                offer_id: row.get(0)?,
                origin: row.get(1)?,
                destination: row.get(2)?,
                cargo: row.get(3)?,
                payout: row.get(4)?,
                eta_hours: row.get(5)?,
                risk: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn estimate_trip_revenue(
    conn: &Connection,
    distance_km: f64,
    contract_bonus: i64,
) -> Result<i64, String> {
    let state = load_state(conn)?;
    Ok((distance_km * state.price_per_km).round() as i64 + contract_bonus)
}

pub fn estimate_trip_costs(
    conn: &Connection,
    fuel_used_liters: f64,
    distance_km: f64,
    repair_reserve: i64,
) -> Result<TripCostBreakdown, String> {
    let state = load_state(conn)?;
    let fuel_cost = (fuel_used_liters * state.diesel_price_per_liter).round() as i64;
    let toll_cost = (distance_km * state.toll_per_km).round() as i64;
    let insurance_cost = (state.insurance_daily_cost / 6).max(45);
    let total_cost = fuel_cost + toll_cost + insurance_cost + repair_reserve;

    Ok(TripCostBreakdown {
        fuel_cost,
        toll_cost,
        insurance_cost,
        total_cost,
    })
}
