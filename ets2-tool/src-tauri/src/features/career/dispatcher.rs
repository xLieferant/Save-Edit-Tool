use std::collections::HashSet;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::features::economy::compensation_models::{
    BaseRateType, CargoType, CompanyPaymentTier, EquipmentType, JobCompensationInput,
    UpsertCompanyPaymentProfileInput, Urgency,
};
use crate::features::economy::compensation_service;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub source: String,
    pub destination: String,
    pub company_id: String,
    pub company_name: String,
    pub origin_country_code: String,
    pub destination_country_code: String,
    pub distance_km: f64,
    pub price_per_km: f64,
    pub cargo: String,
    pub company_payment_tier: CompanyPaymentTier,
    pub company_payment_multiplier: f64,
    pub customer_multiplier: f64,
    pub company_reputation: u16,
    pub company_reputation_multiplier: f64,
    pub country_multiplier: f64,
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

#[derive(Debug, Clone)]
pub struct JobPricingContext {
    pub company_id: String,
    pub company_name: String,
    pub origin_country_code: String,
    pub destination_country_code: String,
}

#[derive(Debug, Clone)]
struct RawJob {
    id: String,
    source: String,
    destination: String,
    company_id: String,
    company_name: String,
    origin_country_code: String,
    destination_country_code: String,
    distance_km: f64,
    price_per_km: f64,
    cargo: String,
    accepted: bool,
    completed: bool,
    progress_km: f64,
}

#[derive(Debug, Clone, Copy)]
struct JobTemplate {
    source: &'static str,
    destination: &'static str,
    distance_km: f64,
    cargo: &'static str,
    company_id: &'static str,
    company_name: &'static str,
    base_rate_type: BaseRateType,
    equipment_type: EquipmentType,
    cargo_type: CargoType,
    urgency: Urgency,
    company_payment_tier: CompanyPaymentTier,
    company_payment_multiplier: f64,
    home_country_code: Option<&'static str>,
    cargo_focus: Option<&'static str>,
    origin_country_code: &'static str,
    destination_country_code: &'static str,
}

const JOB_TEMPLATES: [JobTemplate; 8] = [
    JobTemplate {
        source: "Hamburg",
        destination: "Prague",
        distance_km: 642.0,
        cargo: "Industrial components",
        company_id: "north-axis-logistics",
        company_name: "North Axis Logistics",
        base_rate_type: BaseRateType::QuickJob,
        equipment_type: EquipmentType::QuickJob,
        cargo_type: CargoType::Standard,
        urgency: Urgency::Normal,
        company_payment_tier: CompanyPaymentTier::Standard,
        company_payment_multiplier: 1.00,
        home_country_code: Some("DE"),
        cargo_focus: Some("Industrial components"),
        origin_country_code: "DE",
        destination_country_code: "CZ",
    },
    JobTemplate {
        source: "Berlin",
        destination: "Vienna",
        distance_km: 684.0,
        cargo: "Medical cargo",
        company_id: "meditrans-europe",
        company_name: "MediTrans Europe",
        base_rate_type: BaseRateType::OwnTruck,
        equipment_type: EquipmentType::OwnTruck,
        cargo_type: CargoType::Fragile,
        urgency: Urgency::Priority,
        company_payment_tier: CompanyPaymentTier::Premium,
        company_payment_multiplier: 1.03,
        home_country_code: Some("AT"),
        cargo_focus: Some("Medical cargo"),
        origin_country_code: "DE",
        destination_country_code: "AT",
    },
    JobTemplate {
        source: "Warsaw",
        destination: "Brno",
        distance_km: 518.0,
        cargo: "Dry food pallets",
        company_id: "freshlink-foods",
        company_name: "FreshLink Foods",
        base_rate_type: BaseRateType::OwnTruck,
        equipment_type: EquipmentType::OwnTruck,
        cargo_type: CargoType::Standard,
        urgency: Urgency::Normal,
        company_payment_tier: CompanyPaymentTier::Budget,
        company_payment_multiplier: 0.99,
        home_country_code: Some("PL"),
        cargo_focus: Some("Food logistics"),
        origin_country_code: "PL",
        destination_country_code: "CZ",
    },
    JobTemplate {
        source: "Munich",
        destination: "Genoa",
        distance_km: 734.0,
        cargo: "Machine parts",
        company_id: "alpine-steelworks",
        company_name: "Alpine Steelworks",
        base_rate_type: BaseRateType::OwnTruckOwnTrailer,
        equipment_type: EquipmentType::OwnTruckOwnTrailer,
        cargo_type: CargoType::Valuable,
        urgency: Urgency::Normal,
        company_payment_tier: CompanyPaymentTier::Good,
        company_payment_multiplier: 1.02,
        home_country_code: Some("IT"),
        cargo_focus: Some("Machine parts"),
        origin_country_code: "DE",
        destination_country_code: "IT",
    },
    JobTemplate {
        source: "Dresden",
        destination: "Rotterdam",
        distance_km: 812.0,
        cargo: "Chemical containers",
        company_id: "rhein-chem-cargo",
        company_name: "RheinChem Cargo",
        base_rate_type: BaseRateType::OwnTruckOwnTrailer,
        equipment_type: EquipmentType::OwnTruckOwnTrailer,
        cargo_type: CargoType::Hazardous,
        urgency: Urgency::Priority,
        company_payment_tier: CompanyPaymentTier::Premium,
        company_payment_multiplier: 1.05,
        home_country_code: Some("DE"),
        cargo_focus: Some("Chemical containers"),
        origin_country_code: "DE",
        destination_country_code: "NL",
    },
    JobTemplate {
        source: "Leipzig",
        destination: "Oslo",
        distance_km: 1284.0,
        cargo: "Special cargo",
        company_id: "nordic-heavy-haul",
        company_name: "Nordic Heavy Haul",
        base_rate_type: BaseRateType::OwnTruckOwnTrailer,
        equipment_type: EquipmentType::OwnTruckOwnTrailer,
        cargo_type: CargoType::Oversize,
        urgency: Urgency::Express,
        company_payment_tier: CompanyPaymentTier::Elite,
        company_payment_multiplier: 1.01,
        home_country_code: Some("NO"),
        cargo_focus: Some("Special cargo"),
        origin_country_code: "DE",
        destination_country_code: "NO",
    },
    JobTemplate {
        source: "Frankfurt",
        destination: "Lyon",
        distance_km: 711.0,
        cargo: "Retail freight",
        company_id: "metro-retail-movers",
        company_name: "Metro Retail Movers",
        base_rate_type: BaseRateType::QuickJob,
        equipment_type: EquipmentType::QuickJob,
        cargo_type: CargoType::Standard,
        urgency: Urgency::Normal,
        company_payment_tier: CompanyPaymentTier::Budget,
        company_payment_multiplier: 0.98,
        home_country_code: Some("FR"),
        cargo_focus: Some("Retail freight"),
        origin_country_code: "DE",
        destination_country_code: "FR",
    },
    JobTemplate {
        source: "Kiel",
        destination: "Brussels",
        distance_km: 596.0,
        cargo: "Packaged goods",
        company_id: "atlantic-freight-link",
        company_name: "Atlantic Freight Link",
        base_rate_type: BaseRateType::OwnTruck,
        equipment_type: EquipmentType::OwnTruck,
        cargo_type: CargoType::Refrigerated,
        urgency: Urgency::Priority,
        company_payment_tier: CompanyPaymentTier::Good,
        company_payment_multiplier: 1.00,
        home_country_code: Some("BE"),
        cargo_focus: Some("Packaged goods"),
        origin_country_code: "DE",
        destination_country_code: "BE",
    },
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
            company_id TEXT NOT NULL DEFAULT 'dispatcher-open-market',
            company_name TEXT NOT NULL DEFAULT 'Dispatcher Market',
            origin_country_code TEXT NOT NULL DEFAULT 'DE',
            destination_country_code TEXT NOT NULL DEFAULT 'DE',
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

    ensure_job_columns(conn)?;

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
    let seed = Utc::now().timestamp_millis().unsigned_abs() as u64;

    for index in 0..needed {
        let template = JOB_TEMPLATES[(seed as usize + index) % JOB_TEMPLATES.len()];
        let job_id = format!("dispatch-{}-{}", Utc::now().timestamp_millis(), index);
        compensation_service::upsert_company_payment_profile(
            conn,
            &UpsertCompanyPaymentProfileInput {
                company_id: template.company_id.to_string(),
                company_name: Some(template.company_name.to_string()),
                payment_tier: template.company_payment_tier,
                payment_multiplier: template.company_payment_multiplier,
                home_country_code: template.home_country_code.map(|value| value.to_string()),
                cargo_focus: template.cargo_focus.map(|value| value.to_string()),
            },
        )?;
        let pricing_input = JobCompensationInput {
            company_id: template.company_id.to_string(),
            company_name: Some(template.company_name.to_string()),
            distance_km: template.distance_km,
            base_rate_type: template.base_rate_type,
            equipment_type: template.equipment_type,
            cargo_type: template.cargo_type,
            urgency: template.urgency,
            origin_country_code: template.origin_country_code.to_string(),
            destination_country_code: template.destination_country_code.to_string(),
            market_seed: seed.wrapping_add(index as u64),
        };
        let pricing = compensation_service::calculate_job_compensation(conn, &pricing_input)?;

        conn.execute(
            r#"
            INSERT INTO career_jobs (
                id,
                source,
                destination,
                distance_km,
                price_per_km,
                cargo,
                company_id,
                company_name,
                origin_country_code,
                destination_country_code,
                accepted,
                completed,
                progress_km,
                created_at_utc
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, 0, 0, ?11)
            "#,
            params![
                job_id,
                template.source,
                template.destination,
                template.distance_km,
                pricing.final_rate_per_km,
                template.cargo,
                template.company_id,
                template.company_name,
                template.origin_country_code,
                template.destination_country_code,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    list_jobs(conn, 12)
}

pub fn list_jobs(conn: &Connection, limit: usize) -> Result<Vec<Job>, String> {
    let raw_jobs = load_raw_jobs(conn, limit)?;
    raw_jobs
        .into_iter()
        .map(|raw| hydrate_job(conn, raw))
        .collect::<Result<Vec<_>, _>>()
}

fn load_raw_jobs(conn: &Connection, limit: usize) -> Result<Vec<RawJob>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                source,
                destination,
                company_id,
                company_name,
                origin_country_code,
                destination_country_code,
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
            Ok(RawJob {
                id: row.get(0)?,
                source: row.get(1)?,
                destination: row.get(2)?,
                company_id: row.get(3)?,
                company_name: row.get(4)?,
                origin_country_code: row.get(5)?,
                destination_country_code: row.get(6)?,
                distance_km: row.get(7)?,
                price_per_km: row.get(8)?,
                cargo: row.get(9)?,
                accepted: row.get::<_, i64>(10)? != 0,
                completed: row.get::<_, i64>(11)? != 0,
                progress_km: row.get(12)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn hydrate_job(conn: &Connection, raw: RawJob) -> Result<Job, String> {
    let company_payment = compensation_service::load_company_payment_profile(
        conn,
        &raw.company_id,
        Some(raw.company_name.as_str()),
    )?;
    let company_reputation = compensation_service::load_company_reputation(conn, &raw.company_id)?;
    let customer_multiplier = compensation_service::customer_multiplier(
        company_payment.payment_tier,
        company_payment.payment_multiplier,
    );
    let company_reputation_multiplier =
        compensation_service::reputation_multiplier(company_reputation.reputation);
    let country_multiplier = compensation_service::resolve_country_multiplier(
        conn,
        &raw.origin_country_code,
        &raw.destination_country_code,
    )?;

    Ok(Job {
        id: raw.id,
        source: raw.source,
        destination: raw.destination,
        company_id: raw.company_id,
        company_name: raw.company_name,
        origin_country_code: raw.origin_country_code,
        destination_country_code: raw.destination_country_code,
        distance_km: raw.distance_km,
        price_per_km: raw.price_per_km,
        cargo: raw.cargo,
        company_payment_tier: company_payment.payment_tier,
        company_payment_multiplier: company_payment.payment_multiplier,
        customer_multiplier,
        company_reputation: company_reputation.reputation,
        company_reputation_multiplier,
        country_multiplier,
        accepted: raw.accepted,
        completed: raw.completed,
        progress_km: raw.progress_km,
        estimated_payout: (raw.distance_km * raw.price_per_km).round() as i64,
        remaining_km: (raw.distance_km - raw.progress_km).max(0.0),
    })
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

pub fn load_job_pricing_context(
    conn: &Connection,
    job_id: &str,
) -> Result<Option<JobPricingContext>, String> {
    conn.query_row(
        r#"
        SELECT company_id, company_name, origin_country_code, destination_country_code
        FROM career_jobs
        WHERE id = ?1
        "#,
        [job_id],
        |row| {
            Ok(JobPricingContext {
                company_id: row.get(0)?,
                company_name: row.get(1)?,
                origin_country_code: row.get(2)?,
                destination_country_code: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn find_job_by_id(conn: &Connection, job_id: &str) -> Result<Option<Job>, String> {
    let raw = conn
        .query_row(
        r#"
        SELECT
            id,
            source,
            destination,
            company_id,
            company_name,
            origin_country_code,
            destination_country_code,
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
            Ok(RawJob {
                id: row.get(0)?,
                source: row.get(1)?,
                destination: row.get(2)?,
                company_id: row.get(3)?,
                company_name: row.get(4)?,
                origin_country_code: row.get(5)?,
                destination_country_code: row.get(6)?,
                distance_km: row.get(7)?,
                price_per_km: row.get(8)?,
                cargo: row.get(9)?,
                accepted: row.get::<_, i64>(10)? != 0,
                completed: row.get::<_, i64>(11)? != 0,
                progress_km: row.get(12)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())?;

    match raw {
        Some(raw_job) => hydrate_job(conn, raw_job).map(Some),
        None => Ok(None),
    }
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

fn ensure_job_columns(conn: &Connection) -> Result<(), String> {
    let existing = existing_columns(conn, "career_jobs")?;
    let required = [
        (
            "company_id",
            "TEXT NOT NULL DEFAULT 'dispatcher-open-market'",
        ),
        (
            "company_name",
            "TEXT NOT NULL DEFAULT 'Dispatcher Market'",
        ),
        (
            "origin_country_code",
            "TEXT NOT NULL DEFAULT 'DE'",
        ),
        (
            "destination_country_code",
            "TEXT NOT NULL DEFAULT 'DE'",
        ),
    ];

    for (column, definition) in required {
        if !existing.contains(column) {
            conn.execute(
                &format!("ALTER TABLE career_jobs ADD COLUMN {column} {definition}"),
                [],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn existing_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;

    let mut columns = HashSet::new();
    for row in rows {
        columns.insert(row.map_err(|e| e.to_string())?);
    }

    Ok(columns)
}
