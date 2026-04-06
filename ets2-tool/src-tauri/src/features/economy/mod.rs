use rusqlite::{Connection, params};
use serde::Serialize;

pub mod compensation_models;
pub mod compensation_service;

use crate::features::economy::compensation_models::{
    CompanyPaymentTier, UpsertCompanyPaymentProfileInput,
};

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

        CREATE TABLE IF NOT EXISTS company_reputation (
            company_id TEXT PRIMARY KEY,
            reputation INTEGER NOT NULL CHECK (reputation >= 0 AND reputation <= 1000),
            reliability_streak INTEGER NOT NULL DEFAULT 0,
            completed_jobs INTEGER NOT NULL DEFAULT 0,
            late_jobs INTEGER NOT NULL DEFAULT 0,
            damage_incidents INTEGER NOT NULL DEFAULT 0,
            canceled_jobs INTEGER NOT NULL DEFAULT 0,
            updated_at_utc TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS company_payment_profiles (
            company_id TEXT PRIMARY KEY,
            company_name TEXT,
            payment_tier TEXT NOT NULL DEFAULT 'standard',
            payment_multiplier REAL NOT NULL DEFAULT 1.0,
            home_country_code TEXT,
            cargo_focus TEXT,
            updated_at_utc TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS country_payment_levels (
            country_code TEXT PRIMARY KEY,
            country_name TEXT NOT NULL,
            payment_multiplier REAL NOT NULL
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

    seed_country_payment_levels(conn)?;
    seed_company_payment_profiles(conn)?;

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

fn seed_country_payment_levels(conn: &Connection) -> Result<(), String> {
    let levels = [
        ("CH", "Switzerland", 1.22_f64),
        ("NO", "Norway", 1.20_f64),
        ("DK", "Denmark", 1.16_f64),
        ("SE", "Sweden", 1.15_f64),
        ("DE", "Germany", 1.14_f64),
        ("NL", "Netherlands", 1.13_f64),
        ("BE", "Belgium", 1.11_f64),
        ("AT", "Austria", 1.11_f64),
        ("FR", "France", 1.08_f64),
        ("UK", "United Kingdom", 1.08_f64),
        ("IT", "Italy", 1.06_f64),
        ("ES", "Spain", 1.04_f64),
        ("CZ", "Czechia", 1.00_f64),
        ("PT", "Portugal", 0.99_f64),
        ("PL", "Poland", 0.95_f64),
        ("SK", "Slovakia", 0.95_f64),
        ("HU", "Hungary", 0.94_f64),
        ("HR", "Croatia", 0.94_f64),
        ("RO", "Romania", 0.92_f64),
        ("BG", "Bulgaria", 0.91_f64),
        ("RS", "Serbia", 0.90_f64),
        ("AL", "Albania", 0.88_f64),
        ("MK", "North Macedonia", 0.88_f64),
        ("ME", "Montenegro", 0.88_f64),
    ];

    for (country_code, country_name, payment_multiplier) in levels {
        conn.execute(
            r#"
            INSERT OR IGNORE INTO country_payment_levels (
                country_code,
                country_name,
                payment_multiplier
            )
            VALUES (?1, ?2, ?3)
            "#,
            params![country_code, country_name, payment_multiplier],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn seed_company_payment_profiles(conn: &Connection) -> Result<(), String> {
    let profiles = [
        (
            "north-axis-logistics",
            "North Axis Logistics",
            CompanyPaymentTier::Standard,
            1.00_f64,
            Some("DE".to_string()),
            Some("Industrial components".to_string()),
        ),
        (
            "meditrans-europe",
            "MediTrans Europe",
            CompanyPaymentTier::Premium,
            1.03_f64,
            Some("AT".to_string()),
            Some("Medical cargo".to_string()),
        ),
        (
            "freshlink-foods",
            "FreshLink Foods",
            CompanyPaymentTier::Budget,
            0.99_f64,
            Some("PL".to_string()),
            Some("Food logistics".to_string()),
        ),
        (
            "alpine-steelworks",
            "Alpine Steelworks",
            CompanyPaymentTier::Good,
            1.02_f64,
            Some("IT".to_string()),
            Some("Steel and machine parts".to_string()),
        ),
        (
            "rhein-chem-cargo",
            "RheinChem Cargo",
            CompanyPaymentTier::Premium,
            1.05_f64,
            Some("DE".to_string()),
            Some("Hazardous freight".to_string()),
        ),
        (
            "nordic-heavy-haul",
            "Nordic Heavy Haul",
            CompanyPaymentTier::Elite,
            1.01_f64,
            Some("NO".to_string()),
            Some("Oversize cargo".to_string()),
        ),
        (
            "metro-retail-movers",
            "Metro Retail Movers",
            CompanyPaymentTier::Budget,
            0.98_f64,
            Some("FR".to_string()),
            Some("Retail freight".to_string()),
        ),
        (
            "atlantic-freight-link",
            "Atlantic Freight Link",
            CompanyPaymentTier::Good,
            1.00_f64,
            Some("BE".to_string()),
            Some("Packaged and refrigerated goods".to_string()),
        ),
        (
            "company-north-axis-pharma",
            "North Axis Pharma",
            CompanyPaymentTier::Premium,
            1.04_f64,
            Some("DE".to_string()),
            Some("Medical supplies".to_string()),
        ),
        (
            "company-alpine-steelworks",
            "Alpine Steelworks",
            CompanyPaymentTier::Good,
            1.02_f64,
            Some("DE".to_string()),
            Some("Industrial steel".to_string()),
        ),
        (
            "company-freshlink-foods",
            "FreshLink Foods",
            CompanyPaymentTier::Budget,
            0.99_f64,
            Some("PL".to_string()),
            Some("Fresh produce".to_string()),
        ),
        (
            "open-market",
            "Open Market",
            CompanyPaymentTier::Standard,
            1.00_f64,
            None,
            None,
        ),
        (
            "dispatcher-open-market",
            "Dispatcher Market",
            CompanyPaymentTier::Standard,
            1.00_f64,
            None,
            None,
        ),
    ];

    for (company_id, company_name, payment_tier, payment_multiplier, home_country_code, cargo_focus) in profiles {
        compensation_service::upsert_company_payment_profile(
            conn,
            &UpsertCompanyPaymentProfileInput {
                company_id: company_id.to_string(),
                company_name: Some(company_name.to_string()),
                payment_tier,
                payment_multiplier,
                home_country_code,
                cargo_focus,
            },
        )?;
    }

    Ok(())
}
