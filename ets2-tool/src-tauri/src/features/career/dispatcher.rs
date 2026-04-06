use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use chrono::{Duration, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::features::economy;
use crate::features::economy::compensation_models::{
    BaseRateType, CargoType, CompanyPaymentTier, EquipmentType, JobCompensationInput,
    UpsertCompanyPaymentProfileInput, Urgency,
};
use crate::features::economy::compensation_service;
use crate::shared::sqlite_schema::ensure_columns;

mod generation;
mod models;
mod repo;
mod schema;

use models::{
    DISPATCHER_ACTIVE_JOB_STATUSES, DISPATCHER_MAX_GENERATION_BATCH, DISPATCHER_OPEN_JOB_STATUSES,
    DispatcherGenerationConfigRow, DispatcherJobRow,
};
#[allow(unused_imports)]
pub use models::{
    DispatcherCompanyContact, DispatcherCreateOfferInput, DispatcherGenerationConfigInput,
    DispatcherGenerationRunResult, DispatcherGenerationStatus, DispatcherHistoryResponse,
    DispatcherHistorySummary, DispatcherJobDetails, DispatcherJobFilter,
    DispatcherJobsBySaveContextResponse, DispatcherMarketJob, DispatcherOffer, DispatcherOverview,
    DispatcherRespondToCounterInput, DispatcherSaveContext,
};
use schema::{ensure_dispatcher_generation_config, ensure_dispatcher_tables};

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
    prepare_dispatcher_system(conn)?;

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

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
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

    find_job_by_id(conn, job_id)?.ok_or_else(|| format!("Job not found after accept: {job_id}"))
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
    let required = [
        (
            "company_id",
            "TEXT NOT NULL DEFAULT 'dispatcher-open-market'",
        ),
        ("company_name", "TEXT NOT NULL DEFAULT 'Dispatcher Market'"),
        ("origin_country_code", "TEXT NOT NULL DEFAULT 'DE'"),
        ("destination_country_code", "TEXT NOT NULL DEFAULT 'DE'"),
    ];
    ensure_columns(conn, "career_jobs", &required)?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct DispatcherTemplate {
    company_id: &'static str,
    company_name: &'static str,
    job_type: &'static str,
    cargo_type: CargoType,
    cargo_label: &'static str,
    origin_city: &'static str,
    origin_country: &'static str,
    destination_city: &'static str,
    destination_country: &'static str,
    distance_km: f64,
    cargo_mass_kg: f64,
    urgency: Urgency,
    urgency_level: &'static str,
    difficulty_level: &'static str,
    equipment_type_required: &'static str,
    trailer_type_required: Option<&'static str>,
    reputation_required: u16,
    payment_tier: CompanyPaymentTier,
    payment_multiplier: f64,
}

const DISPATCHER_TEMPLATES: [DispatcherTemplate; 12] = [
    DispatcherTemplate {
        company_id: "north-axis-logistics",
        company_name: "North Axis Logistics",
        job_type: "quick_job",
        cargo_type: CargoType::Standard,
        cargo_label: "Industrial Components",
        origin_city: "Hamburg",
        origin_country: "DE",
        destination_city: "Prague",
        destination_country: "CZ",
        distance_km: 642.0,
        cargo_mass_kg: 12100.0,
        urgency: Urgency::Normal,
        urgency_level: "normal",
        difficulty_level: "easy",
        equipment_type_required: "quick_job",
        trailer_type_required: None,
        reputation_required: 120,
        payment_tier: CompanyPaymentTier::Standard,
        payment_multiplier: 1.00,
    },
    DispatcherTemplate {
        company_id: "meditrans-europe",
        company_name: "MediTrans Europe",
        job_type: "freight_market",
        cargo_type: CargoType::Fragile,
        cargo_label: "Medical Cargo",
        origin_city: "Berlin",
        origin_country: "DE",
        destination_city: "Vienna",
        destination_country: "AT",
        distance_km: 684.0,
        cargo_mass_kg: 9200.0,
        urgency: Urgency::Priority,
        urgency_level: "high",
        difficulty_level: "normal",
        equipment_type_required: "own_truck",
        trailer_type_required: Some("box"),
        reputation_required: 350,
        payment_tier: CompanyPaymentTier::Premium,
        payment_multiplier: 1.03,
    },
    DispatcherTemplate {
        company_id: "rhein-chem-cargo",
        company_name: "RheinChem Cargo",
        job_type: "cargo_market",
        cargo_type: CargoType::Hazardous,
        cargo_label: "Chemical Containers",
        origin_city: "Dresden",
        origin_country: "DE",
        destination_city: "Rotterdam",
        destination_country: "NL",
        distance_km: 812.0,
        cargo_mass_kg: 20500.0,
        urgency: Urgency::Priority,
        urgency_level: "high",
        difficulty_level: "hard",
        equipment_type_required: "own_truck_trailer",
        trailer_type_required: Some("adr_tanker"),
        reputation_required: 520,
        payment_tier: CompanyPaymentTier::Premium,
        payment_multiplier: 1.05,
    },
    DispatcherTemplate {
        company_id: "nordic-heavy-haul",
        company_name: "Nordic Heavy Haul",
        job_type: "premium_special",
        cargo_type: CargoType::Oversize,
        cargo_label: "Oversize Turbine Parts",
        origin_city: "Leipzig",
        origin_country: "DE",
        destination_city: "Oslo",
        destination_country: "NO",
        distance_km: 1284.0,
        cargo_mass_kg: 35000.0,
        urgency: Urgency::Express,
        urgency_level: "critical",
        difficulty_level: "expert",
        equipment_type_required: "own_truck_trailer",
        trailer_type_required: Some("heavy_haul"),
        reputation_required: 700,
        payment_tier: CompanyPaymentTier::Elite,
        payment_multiplier: 1.02,
    },
    DispatcherTemplate {
        company_id: "freshlink-foods",
        company_name: "FreshLink Foods",
        job_type: "company_contract",
        cargo_type: CargoType::Refrigerated,
        cargo_label: "Frozen Goods",
        origin_city: "Brno",
        origin_country: "CZ",
        destination_city: "Munich",
        destination_country: "DE",
        distance_km: 526.0,
        cargo_mass_kg: 17000.0,
        urgency: Urgency::Priority,
        urgency_level: "high",
        difficulty_level: "hard",
        equipment_type_required: "own_truck_trailer",
        trailer_type_required: Some("refrigerated"),
        reputation_required: 430,
        payment_tier: CompanyPaymentTier::Good,
        payment_multiplier: 1.01,
    },
    DispatcherTemplate {
        company_id: "atlantic-freight-link",
        company_name: "Atlantic Freight Link",
        job_type: "freight_market",
        cargo_type: CargoType::Standard,
        cargo_label: "Packaged Goods",
        origin_city: "Kiel",
        origin_country: "DE",
        destination_city: "Brussels",
        destination_country: "BE",
        distance_km: 596.0,
        cargo_mass_kg: 13700.0,
        urgency: Urgency::Normal,
        urgency_level: "normal",
        difficulty_level: "normal",
        equipment_type_required: "own_truck",
        trailer_type_required: Some("curtain"),
        reputation_required: 240,
        payment_tier: CompanyPaymentTier::Good,
        payment_multiplier: 1.00,
    },
    DispatcherTemplate {
        company_id: "metro-retail-movers",
        company_name: "Metro Retail Movers",
        job_type: "quick_job",
        cargo_type: CargoType::Standard,
        cargo_label: "Retail Freight",
        origin_city: "Frankfurt",
        origin_country: "DE",
        destination_city: "Lyon",
        destination_country: "FR",
        distance_km: 711.0,
        cargo_mass_kg: 11200.0,
        urgency: Urgency::Normal,
        urgency_level: "normal",
        difficulty_level: "easy",
        equipment_type_required: "quick_job",
        trailer_type_required: None,
        reputation_required: 100,
        payment_tier: CompanyPaymentTier::Budget,
        payment_multiplier: 0.99,
    },
    DispatcherTemplate {
        company_id: "alpine-steelworks",
        company_name: "Alpine Steelworks",
        job_type: "cargo_market",
        cargo_type: CargoType::Valuable,
        cargo_label: "Machine Parts",
        origin_city: "Munich",
        origin_country: "DE",
        destination_city: "Genoa",
        destination_country: "IT",
        distance_km: 734.0,
        cargo_mass_kg: 18400.0,
        urgency: Urgency::Normal,
        urgency_level: "normal",
        difficulty_level: "hard",
        equipment_type_required: "own_truck_trailer",
        trailer_type_required: Some("flatbed"),
        reputation_required: 410,
        payment_tier: CompanyPaymentTier::Good,
        payment_multiplier: 1.02,
    },
    DispatcherTemplate {
        company_id: "freshlink-foods",
        company_name: "FreshLink Foods",
        job_type: "freight_market",
        cargo_type: CargoType::Refrigerated,
        cargo_label: "Seafood Pallets",
        origin_city: "Oslo",
        origin_country: "NO",
        destination_city: "Hamburg",
        destination_country: "DE",
        distance_km: 1037.0,
        cargo_mass_kg: 19400.0,
        urgency: Urgency::Priority,
        urgency_level: "high",
        difficulty_level: "hard",
        equipment_type_required: "own_truck_trailer",
        trailer_type_required: Some("refrigerated"),
        reputation_required: 540,
        payment_tier: CompanyPaymentTier::Premium,
        payment_multiplier: 1.05,
    },
    DispatcherTemplate {
        company_id: "meditrans-europe",
        company_name: "MediTrans Europe",
        job_type: "premium_special",
        cargo_type: CargoType::Valuable,
        cargo_label: "Biotech Equipment",
        origin_city: "Stuttgart",
        origin_country: "DE",
        destination_city: "Zurich",
        destination_country: "CH",
        distance_km: 323.0,
        cargo_mass_kg: 8400.0,
        urgency: Urgency::Express,
        urgency_level: "critical",
        difficulty_level: "expert",
        equipment_type_required: "own_truck",
        trailer_type_required: Some("secure_box"),
        reputation_required: 720,
        payment_tier: CompanyPaymentTier::Elite,
        payment_multiplier: 1.08,
    },
    DispatcherTemplate {
        company_id: "north-axis-logistics",
        company_name: "North Axis Logistics",
        job_type: "company_contract",
        cargo_type: CargoType::Standard,
        cargo_label: "Warehouse Relocation",
        origin_city: "Le Havre",
        origin_country: "FR",
        destination_city: "Duisburg",
        destination_country: "DE",
        distance_km: 584.0,
        cargo_mass_kg: 16400.0,
        urgency: Urgency::Priority,
        urgency_level: "high",
        difficulty_level: "normal",
        equipment_type_required: "own_truck",
        trailer_type_required: Some("box"),
        reputation_required: 390,
        payment_tier: CompanyPaymentTier::Standard,
        payment_multiplier: 1.01,
    },
    DispatcherTemplate {
        company_id: "rhein-chem-cargo",
        company_name: "RheinChem Cargo",
        job_type: "premium_special",
        cargo_type: CargoType::Hazardous,
        cargo_label: "Lab Reagents",
        origin_city: "Basel",
        origin_country: "CH",
        destination_city: "Berlin",
        destination_country: "DE",
        distance_km: 841.0,
        cargo_mass_kg: 15400.0,
        urgency: Urgency::Express,
        urgency_level: "critical",
        difficulty_level: "expert",
        equipment_type_required: "own_truck_trailer",
        trailer_type_required: Some("adr_box"),
        reputation_required: 740,
        payment_tier: CompanyPaymentTier::Elite,
        payment_multiplier: 1.07,
    },
];

fn seed_dispatcher_contacts(conn: &Connection) -> Result<(), String> {
    for template in DISPATCHER_TEMPLATES {
        compensation_service::upsert_company_payment_profile(
            conn,
            &UpsertCompanyPaymentProfileInput {
                company_id: template.company_id.to_string(),
                company_name: Some(template.company_name.to_string()),
                payment_tier: template.payment_tier,
                payment_multiplier: template.payment_multiplier,
                home_country_code: Some(template.origin_country.to_string()),
                cargo_focus: Some(template.cargo_label.to_string()),
            },
        )?;
    }

    Ok(())
}

fn prepare_dispatcher_system(conn: &Connection) -> Result<(), String> {
    ensure_dispatcher_tables(conn)?;
    seed_dispatcher_contacts(conn)?;
    Ok(())
}

fn load_dispatcher_generation_config(
    conn: &Connection,
) -> Result<DispatcherGenerationConfigRow, String> {
    ensure_dispatcher_generation_config(conn)?;
    conn.query_row(
        r#"
        SELECT interval_minutes, max_open_jobs, last_generated_at_utc, last_cleanup_at_utc
        FROM dispatcher_generation_config
        WHERE id = 1
        "#,
        [],
        |row| {
            Ok(DispatcherGenerationConfigRow {
                interval_minutes: row.get(0)?,
                max_open_jobs: row.get(1)?,
                last_generated_at_utc: row.get(2)?,
                last_cleanup_at_utc: row.get(3)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

fn clamp_dispatcher_interval_minutes(value: i64) -> i64 {
    match value {
        ..=5 => 5,
        6..=10 => 10,
        _ => 15,
    }
}

fn clamp_dispatcher_max_open_jobs(value: i64) -> i64 {
    value.clamp(12, 36)
}

fn apply_dispatcher_generation_config(
    conn: &Connection,
    config: Option<DispatcherGenerationConfigInput>,
) -> Result<DispatcherGenerationConfigRow, String> {
    ensure_dispatcher_generation_config(conn)?;

    if let Some(config) = config {
        let current = load_dispatcher_generation_config(conn)?;
        let interval_minutes = config
            .interval_minutes
            .map(clamp_dispatcher_interval_minutes)
            .unwrap_or(current.interval_minutes);
        let max_open_jobs = config
            .max_open_jobs
            .map(clamp_dispatcher_max_open_jobs)
            .unwrap_or(current.max_open_jobs);
        conn.execute(
            r#"
            UPDATE dispatcher_generation_config
            SET interval_minutes = ?1, max_open_jobs = ?2
            WHERE id = 1
            "#,
            params![interval_minutes, max_open_jobs],
        )
        .map_err(|e| e.to_string())?;
    }

    load_dispatcher_generation_config(conn)
}

fn count_dispatcher_jobs_by_status(
    conn: &Connection,
    statuses: &[&str],
    save_context: &DispatcherSaveContext,
    source_type: Option<&str>,
) -> Result<i64, String> {
    if !save_context.is_ready() {
        return Ok(0);
    }

    let in_clause = statuses
        .iter()
        .map(|status| format!("'{}'", status))
        .collect::<Vec<_>>()
        .join(", ");
    let source_clause = source_type
        .map(|value| format!(" AND source_type = '{}'", value))
        .unwrap_or_default();
    let sql = format!(
        "
        SELECT COUNT(*)
        FROM dispatcher_jobs
        WHERE status IN ({in_clause})
          AND (?1 IS NULL OR profile_reference = ?1)
          AND (?2 IS NULL OR save_reference = ?2)
          {source_clause}
        "
    );

    conn.query_row(
        &sql,
        params![
            save_context.profile_reference.as_deref(),
            save_context.save_reference.as_deref()
        ],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

fn count_dispatcher_company_jobs(
    conn: &Connection,
    company_id: &str,
    statuses: &[&str],
    save_context: &DispatcherSaveContext,
) -> Result<i64, String> {
    if !save_context.is_ready() {
        return Ok(0);
    }

    let in_clause = statuses
        .iter()
        .map(|status| format!("'{}'", status))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "
        SELECT COUNT(*)
        FROM dispatcher_jobs
        WHERE company_id = ?1
          AND status IN ({in_clause})
          AND (?2 IS NULL OR profile_reference = ?2)
          AND (?3 IS NULL OR save_reference = ?3)
        "
    );

    conn.query_row(
        &sql,
        params![
            company_id,
            save_context.profile_reference.as_deref(),
            save_context.save_reference.as_deref()
        ],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

fn dispatcher_generation_due(last_generated_at_utc: Option<&str>, interval_minutes: i64) -> bool {
    let Some(last_generated_at_utc) = last_generated_at_utc else {
        return true;
    };
    let Some(last_generated_at) = parse_dispatcher_datetime(last_generated_at_utc) else {
        return true;
    };
    Utc::now() >= last_generated_at + Duration::minutes(interval_minutes.max(5))
}

fn dispatcher_generation_batch_size(max_open_jobs: i64) -> usize {
    if max_open_jobs <= 12 {
        3
    } else {
        DISPATCHER_MAX_GENERATION_BATCH
    }
}

fn parse_dispatcher_datetime(value: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date_time| date_time.with_timezone(&Utc))
}

fn build_dispatcher_generation_status(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationStatus, String> {
    let config = load_dispatcher_generation_config(conn)?;
    let open_generated_jobs = count_dispatcher_jobs_by_status(
        conn,
        DISPATCHER_OPEN_JOB_STATUSES,
        save_context,
        Some("generated"),
    )?;
    let open_total_jobs =
        count_dispatcher_jobs_by_status(conn, DISPATCHER_OPEN_JOB_STATUSES, save_context, None)?;
    let next_generation_at_utc = if !save_context.is_ready() {
        None
    } else if open_total_jobs == 0 || config.last_generated_at_utc.is_none() {
        Some(Utc::now().to_rfc3339())
    } else {
        config
            .last_generated_at_utc
            .as_deref()
            .and_then(parse_dispatcher_datetime)
            .map(|last| (last + Duration::minutes(config.interval_minutes.max(5))).to_rfc3339())
    };

    Ok(DispatcherGenerationStatus {
        interval_minutes: config.interval_minutes,
        max_open_jobs: config.max_open_jobs,
        open_generated_jobs,
        open_total_jobs,
        last_generated_at_utc: config.last_generated_at_utc,
        last_cleanup_at_utc: config.last_cleanup_at_utc,
        next_generation_at_utc,
        current_context: save_context.clone(),
        save_link_active: save_context.is_ready(),
    })
}

fn dispatcher_job_type_variants(template: DispatcherTemplate) -> Vec<&'static str> {
    match template.equipment_type_required {
        "quick_job" => vec!["quick_job"],
        "own_truck" => vec!["freight_market", "company_contract"],
        "own_truck_trailer" => {
            if matches!(
                template.payment_tier,
                CompanyPaymentTier::Premium | CompanyPaymentTier::Elite
            ) {
                vec!["cargo_market", "company_contract", "premium_special"]
            } else {
                vec!["cargo_market", "company_contract"]
            }
        }
        _ => vec![template.job_type],
    }
}

fn dispatcher_job_signature(
    company_id: &str,
    origin_city: &str,
    destination_city: &str,
    cargo_type: &str,
    job_type: &str,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        company_id.to_ascii_lowercase(),
        origin_city.to_ascii_lowercase(),
        destination_city.to_ascii_lowercase(),
        cargo_type.to_ascii_lowercase(),
        job_type.to_ascii_lowercase()
    )
}

fn build_dispatcher_route_reference(
    company_id: &str,
    origin_country: &str,
    origin_city: &str,
    destination_country: &str,
    destination_city: &str,
    job_type: &str,
) -> String {
    let slug = |value: &str| {
        value
            .chars()
            .map(|character| match character {
                'a'..='z' | '0'..='9' => character,
                'A'..='Z' => character.to_ascii_lowercase(),
                _ => '_',
            })
            .collect::<String>()
    };

    format!(
        "{}__{}_{}__{}_{}__{}",
        slug(company_id),
        slug(origin_country),
        slug(origin_city),
        slug(destination_country),
        slug(destination_city),
        slug(job_type)
    )
}

fn dispatcher_average_speed_kmh(urgency: Urgency) -> f64 {
    match urgency {
        Urgency::Express => 74.0,
        Urgency::Priority => 68.0,
        Urgency::Normal => 64.0,
    }
}

fn load_dispatcher_open_signatures(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<HashSet<String>, String> {
    if !save_context.is_ready() {
        return Ok(HashSet::new());
    }

    let mut stmt = conn
        .prepare(
            r#"
            SELECT company_id, origin_city, destination_city, cargo_type, job_type
            FROM dispatcher_jobs
            WHERE status = 'open'
              AND (?1 IS NULL OR profile_reference = ?1)
              AND (?2 IS NULL OR save_reference = ?2)
            "#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params![
                save_context.profile_reference.as_deref(),
                save_context.save_reference.as_deref()
            ],
            |row| {
                let company_id: String = row.get(0)?;
                let origin_city: String = row.get(1)?;
                let destination_city: String = row.get(2)?;
                let cargo_type: String = row.get(3)?;
                let job_type: String = row.get(4)?;
                Ok(dispatcher_job_signature(
                    &company_id,
                    &origin_city,
                    &destination_city,
                    &cargo_type,
                    &job_type,
                ))
            },
        )
        .map_err(|e| e.to_string())?;

    let mut signatures = HashSet::new();
    for row in rows {
        signatures.insert(row.map_err(|e| e.to_string())?);
    }

    Ok(signatures)
}

fn generate_dispatcher_jobs_for_context(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
    count: usize,
) -> Result<usize, String> {
    if !save_context.is_ready() || count == 0 {
        return Ok(0);
    }

    let economy_state = economy::load_state(conn)?;
    let mut signatures = load_dispatcher_open_signatures(conn, save_context)?;
    let base_seed = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        save_context.profile_reference.hash(&mut hasher);
        save_context.save_reference.hash(&mut hasher);
        save_context.save_session_id.hash(&mut hasher);
        Utc::now().timestamp_millis().hash(&mut hasher);
        hasher.finish()
    };

    let mut created = 0usize;
    let max_attempts = (count * 12).max(DISPATCHER_TEMPLATES.len() * 4);

    for attempt in 0..max_attempts {
        if created >= count {
            break;
        }

        let template_index = ((base_seed as usize) + attempt) % DISPATCHER_TEMPLATES.len();
        let template = DISPATCHER_TEMPLATES[template_index];
        let job_type_variants = dispatcher_job_type_variants(template);
        let variant_index = ((base_seed as usize) + attempt) % job_type_variants.len();
        let job_type = job_type_variants[variant_index];
        let cargo_type = cargo_type_to_db(template.cargo_type);
        let signature = dispatcher_job_signature(
            template.company_id,
            template.origin_city,
            template.destination_city,
            &cargo_type,
            job_type,
        );
        if signatures.contains(&signature) {
            continue;
        }

        let pricing_input = JobCompensationInput {
            company_id: template.company_id.to_string(),
            company_name: Some(template.company_name.to_string()),
            distance_km: template.distance_km,
            base_rate_type: base_rate_type_for_dispatcher_job(job_type),
            equipment_type: equipment_type_for_dispatcher_job(template.equipment_type_required),
            cargo_type: template.cargo_type,
            urgency: template.urgency,
            origin_country_code: template.origin_country.to_string(),
            destination_country_code: template.destination_country.to_string(),
            market_seed: base_seed + attempt as u64,
        };
        let pricing = compensation_service::calculate_job_compensation(conn, &pricing_input)?;
        let final_rate = pricing.final_rate_per_km
            * dispatcher_job_type_modifier(job_type)
            * dispatcher_difficulty_modifier(template.difficulty_level);
        let total_reward = (template.distance_km * final_rate).round() as i64;
        let fuel_cost =
            (template.distance_km * 0.31 * economy_state.diesel_price_per_liter).round() as i64;
        let toll_cost = (template.distance_km * economy_state.toll_per_km).round() as i64;
        let insurance_cost = (economy_state.insurance_daily_cost / 6).max(45);
        let profit_estimate = total_reward - fuel_cost - toll_cost - insurance_cost;
        let estimated_duration_minutes =
            ((template.distance_km / dispatcher_average_speed_kmh(template.urgency)) * 60.0).ceil()
                as i64
                + 35;
        let now = Utc::now();
        let expires_at = now
            + Duration::hours(match template.urgency {
                Urgency::Express => 8,
                Urgency::Priority => 14,
                Urgency::Normal => 20,
            });
        let route_reference = build_dispatcher_route_reference(
            template.company_id,
            template.origin_country,
            template.origin_city,
            template.destination_country,
            template.destination_city,
            job_type,
        );

        conn.execute(
            r#"
            INSERT INTO dispatcher_jobs (
                id,
                source_type,
                company_id,
                company_name,
                job_type,
                cargo_type,
                origin_city,
                origin_country,
                destination_city,
                destination_country,
                distance_km,
                cargo_mass_kg,
                urgency_level,
                difficulty_level,
                equipment_type_required,
                trailer_type_required,
                base_rate_per_km,
                calculated_rate_per_km,
                total_reward,
                estimated_duration_minutes,
                payment_tier_snapshot,
                payment_multiplier_snapshot,
                country_multiplier_snapshot,
                reputation_multiplier_snapshot,
                cargo_multiplier_snapshot,
                urgency_multiplier_snapshot,
                equipment_multiplier_snapshot,
                market_variation_snapshot,
                customer_multiplier_snapshot,
                company_reputation,
                fuel_cost_estimate,
                profit_estimate,
                risk_note,
                bonus_note,
                expires_at_utc,
                status,
                progress_km,
                profile_reference,
                save_reference,
                quicksave_reference,
                save_session_id,
                route_reference,
                ets2_job_link_status,
                accepted_at_utc,
                completed_at_utc,
                created_at_utc,
                updated_at_utc
            )
            VALUES (
                ?1, 'generated', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31,
                ?32, ?33, ?34, 'open', 0, ?35, ?36, ?37, ?38, ?39, 'pending_route', NULL, NULL,
                ?40, ?40
            )
            "#,
            params![
                build_dispatcher_job_id(
                    template.company_id,
                    (base_seed as usize) + attempt,
                    created
                ),
                template.company_id,
                template.company_name,
                job_type,
                cargo_type,
                template.origin_city,
                template.origin_country,
                template.destination_city,
                template.destination_country,
                template.distance_km,
                template.cargo_mass_kg,
                template.urgency_level,
                template.difficulty_level,
                template.equipment_type_required,
                template.trailer_type_required,
                pricing.base_rate_per_km,
                final_rate,
                total_reward,
                estimated_duration_minutes,
                payment_tier_to_db(template.payment_tier),
                template.payment_multiplier,
                pricing.country_multiplier,
                pricing.company_reputation_multiplier,
                pricing.cargo_multiplier,
                pricing.urgency_multiplier,
                pricing.equipment_multiplier,
                pricing.market_variation,
                pricing.customer_multiplier,
                pricing.company_reputation as i64,
                fuel_cost,
                profit_estimate,
                dispatcher_risk_note(template.difficulty_level, template.urgency_level),
                dispatcher_bonus_note(template.payment_tier, template.equipment_type_required),
                expires_at.to_rfc3339(),
                save_context.profile_reference.as_deref(),
                save_context.save_reference.as_deref(),
                save_context.quicksave_reference.as_deref(),
                save_context.save_session_id.as_deref(),
                route_reference,
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| e.to_string())?;

        signatures.insert(signature);
        created += 1;
    }

    Ok(created)
}

fn ensure_dispatcher_market_jobs(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
    force: bool,
) -> Result<DispatcherGenerationRunResult, String> {
    let expired_count = expire_dispatcher_market_jobs(conn)?;
    let config = load_dispatcher_generation_config(conn)?;
    let mut market_changed = expired_count > 0;

    if save_context.is_ready() {
        let open_total = count_dispatcher_jobs_by_status(
            conn,
            DISPATCHER_OPEN_JOB_STATUSES,
            save_context,
            None,
        )?;
        let should_generate = force
            || open_total == 0
            || (open_total < config.max_open_jobs
                && dispatcher_generation_due(
                    config.last_generated_at_utc.as_deref(),
                    config.interval_minutes,
                ));

        if should_generate && open_total < config.max_open_jobs {
            let needed = (config.max_open_jobs - open_total).max(0) as usize;
            let batch_size = dispatcher_generation_batch_size(config.max_open_jobs);
            let generation_target = if force || open_total == 0 {
                needed
            } else {
                needed.min(batch_size)
            };
            let created =
                generate_dispatcher_jobs_for_context(conn, save_context, generation_target)?;
            if created > 0 {
                let now = Utc::now().to_rfc3339();
                conn.execute(
                    "UPDATE dispatcher_generation_config SET last_generated_at_utc = ?1 WHERE id = 1",
                    params![now],
                )
                .map_err(|e| e.to_string())?;
                market_changed = true;
            }
        }
    }

    Ok(DispatcherGenerationRunResult {
        status: build_dispatcher_generation_status(conn, save_context)?,
        market_changed,
    })
}

fn expire_dispatcher_market_jobs(conn: &Connection) -> Result<usize, String> {
    let now = Utc::now().to_rfc3339();
    let expired = conn
        .execute(
            "UPDATE dispatcher_jobs SET status = 'expired', updated_at_utc = ?1 WHERE status = 'open' AND expires_at_utc IS NOT NULL AND expires_at_utc < ?1",
            [now.clone()],
        )
        .map_err(|e| e.to_string())?;

    if expired > 0 {
        conn.execute(
            "UPDATE dispatcher_generation_config SET last_cleanup_at_utc = ?1 WHERE id = 1",
            params![now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(expired)
}

fn dispatcher_risk_note(difficulty: &str, urgency: &str) -> Option<String> {
    let d = difficulty.to_ascii_lowercase();
    let u = urgency.to_ascii_lowercase();
    if d == "expert" && u == "critical" {
        Some("high_deadline_and_damage_risk".to_string())
    } else if d == "hard" || u == "high" {
        Some("tight_dispatch_margin".to_string())
    } else {
        None
    }
}

fn dispatcher_bonus_note(tier: CompanyPaymentTier, equipment: &str) -> Option<String> {
    if matches!(
        tier,
        CompanyPaymentTier::Premium | CompanyPaymentTier::Elite
    ) && equipment.eq_ignore_ascii_case("own_truck_trailer")
    {
        Some("premium_client_with_own_trailer_bonus".to_string())
    } else {
        None
    }
}

fn build_dispatcher_job_id(company_id: &str, seed: usize, index: usize) -> String {
    format!(
        "dispatcher-{}-{}-{}",
        Utc::now().timestamp_millis(),
        (seed + index) % 997,
        company_id.bytes().fold(0_u64, |acc, byte| acc
            .wrapping_mul(131)
            .wrapping_add(byte as u64))
            % 991
    )
}

fn map_dispatcher_job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DispatcherJobRow> {
    Ok(DispatcherJobRow {
        id: row.get("id")?,
        source_type: row.get("source_type")?,
        company_id: row.get("company_id")?,
        company_name: row.get("company_name")?,
        job_type: row.get("job_type")?,
        cargo_type: row.get("cargo_type")?,
        origin_city: row.get("origin_city")?,
        origin_country: row.get("origin_country")?,
        destination_city: row.get("destination_city")?,
        destination_country: row.get("destination_country")?,
        distance_km: row.get("distance_km")?,
        cargo_mass_kg: row.get("cargo_mass_kg")?,
        urgency_level: row.get("urgency_level")?,
        difficulty_level: row.get("difficulty_level")?,
        equipment_type_required: row.get("equipment_type_required")?,
        trailer_type_required: row.get("trailer_type_required")?,
        base_rate_per_km: row.get("base_rate_per_km")?,
        calculated_rate_per_km: row.get("calculated_rate_per_km")?,
        total_reward: row.get("total_reward")?,
        estimated_duration_minutes: row.get("estimated_duration_minutes")?,
        payment_tier_snapshot: row.get("payment_tier_snapshot")?,
        payment_multiplier_snapshot: row.get("payment_multiplier_snapshot")?,
        country_multiplier_snapshot: row.get("country_multiplier_snapshot")?,
        reputation_multiplier_snapshot: row.get("reputation_multiplier_snapshot")?,
        cargo_multiplier_snapshot: row.get("cargo_multiplier_snapshot")?,
        urgency_multiplier_snapshot: row.get("urgency_multiplier_snapshot")?,
        equipment_multiplier_snapshot: row.get("equipment_multiplier_snapshot")?,
        market_variation_snapshot: row.get("market_variation_snapshot")?,
        customer_multiplier_snapshot: row.get("customer_multiplier_snapshot")?,
        company_reputation: row.get("company_reputation")?,
        fuel_cost_estimate: row.get("fuel_cost_estimate")?,
        profit_estimate: row.get("profit_estimate")?,
        risk_note: row.get("risk_note")?,
        bonus_note: row.get("bonus_note")?,
        expires_at_utc: row.get("expires_at_utc")?,
        status: row.get("status")?,
        progress_km: row.get("progress_km")?,
        profile_reference: row.get("profile_reference")?,
        save_reference: row.get("save_reference")?,
        quicksave_reference: row.get("quicksave_reference")?,
        save_session_id: row.get("save_session_id")?,
        route_reference: row.get("route_reference")?,
        ets2_job_link_status: row.get("ets2_job_link_status")?,
        last_error_code: row.get("last_error_code")?,
        last_error_message: row.get("last_error_message")?,
        accepted_at_utc: row.get("accepted_at_utc")?,
        completed_at_utc: row.get("completed_at_utc")?,
        created_at_utc: row.get("created_at_utc")?,
        updated_at_utc: row.get("updated_at_utc")?,
    })
}

fn load_dispatcher_job_by_id(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<Option<DispatcherJobRow>, String> {
    if !save_context.is_ready() {
        return Ok(None);
    }

    let mut stmt = conn
        .prepare(
            r#"
            SELECT *
            FROM dispatcher_jobs
            WHERE id = ?1
              AND (?2 IS NULL OR profile_reference = ?2)
              AND (?3 IS NULL OR save_reference = ?3)
            "#,
        )
        .map_err(|e| e.to_string())?;
    stmt.query_row(
        params![
            job_id,
            save_context.profile_reference.as_deref(),
            save_context.save_reference.as_deref()
        ],
        map_dispatcher_job_row,
    )
    .optional()
    .map_err(|e| e.to_string())
}

fn load_dispatcher_job_by_id_any(
    conn: &Connection,
    job_id: &str,
) -> Result<Option<DispatcherJobRow>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT *
            FROM dispatcher_jobs
            WHERE id = ?1
            "#,
        )
        .map_err(|e| e.to_string())?;
    stmt.query_row([job_id], map_dispatcher_job_row)
        .optional()
        .map_err(|e| e.to_string())
}

fn list_dispatcher_jobs_by_status(
    conn: &Connection,
    statuses: &[&str],
    limit: usize,
    save_context: &DispatcherSaveContext,
) -> Result<Vec<DispatcherJobRow>, String> {
    if !save_context.is_ready() {
        return Ok(Vec::new());
    }

    let in_clause = statuses
        .iter()
        .map(|status| format!("'{}'", status))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "
        SELECT *
        FROM dispatcher_jobs
        WHERE status IN ({in_clause})
          AND (?1 IS NULL OR profile_reference = ?1)
          AND (?2 IS NULL OR save_reference = ?2)
        ORDER BY created_at_utc DESC
        LIMIT ?3
        "
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params![
                save_context.profile_reference.as_deref(),
                save_context.save_reference.as_deref(),
                limit as i64
            ],
            map_dispatcher_job_row,
        )
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn to_dispatcher_market_job(row: DispatcherJobRow) -> DispatcherMarketJob {
    let payment_tier_snapshot = row.payment_tier_snapshot.clone();
    let status = row.status.clone();
    let distance_km = row.distance_km;
    let linked_to_active_save = row.profile_reference.is_some() && row.save_reference.is_some();

    DispatcherMarketJob {
        id: row.id,
        source_type: row.source_type,
        company_id: row.company_id,
        company_name: row.company_name,
        job_type: row.job_type,
        cargo_type: row.cargo_type,
        origin_city: row.origin_city,
        origin_country: row.origin_country,
        destination_city: row.destination_city,
        destination_country: row.destination_country,
        distance_km,
        cargo_mass_kg: row.cargo_mass_kg,
        urgency_level: row.urgency_level,
        difficulty_level: row.difficulty_level,
        equipment_type_required: row.equipment_type_required,
        trailer_type_required: row.trailer_type_required,
        base_rate_per_km: row.base_rate_per_km,
        calculated_rate_per_km: row.calculated_rate_per_km,
        total_reward: row.total_reward,
        estimated_duration_minutes: row.estimated_duration_minutes,
        payment_tier_snapshot,
        payment_tier: row.payment_tier_snapshot,
        company_multiplier_snapshot: row.payment_multiplier_snapshot,
        company_reputation: row.company_reputation.clamp(0, 1000) as u16,
        country_multiplier_snapshot: row.country_multiplier_snapshot,
        reputation_multiplier_snapshot: row.reputation_multiplier_snapshot,
        cargo_multiplier_snapshot: row.cargo_multiplier_snapshot,
        urgency_multiplier_snapshot: row.urgency_multiplier_snapshot,
        equipment_multiplier_snapshot: row.equipment_multiplier_snapshot,
        market_variation_snapshot: row.market_variation_snapshot,
        customer_multiplier_snapshot: row.customer_multiplier_snapshot,
        fuel_cost_estimate: row.fuel_cost_estimate,
        profit_estimate: row.profit_estimate,
        risk_note: row.risk_note,
        bonus_note: row.bonus_note,
        expires_at_utc: row.expires_at_utc,
        status,
        dispatcher_status: row.status,
        progress_km: row.progress_km,
        route_distance_km: distance_km,
        profile_reference: row.profile_reference,
        save_reference: row.save_reference,
        quicksave_reference: row.quicksave_reference,
        save_session_id: row.save_session_id,
        linked_to_active_save,
        route_reference: row.route_reference,
        ets2_job_link_status: row.ets2_job_link_status,
        last_error_code: row.last_error_code,
        last_error_message: row.last_error_message,
        accepted_at_utc: row.accepted_at_utc,
        completed_at_utc: row.completed_at_utc,
    }
}

fn matches_filter_text(value: &str, needle: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

pub fn dispatcher_get_dispatcher_overview(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherOverview, String> {
    prepare_dispatcher_system(conn)?;
    expire_dispatcher_offers(conn)?;
    let generation = ensure_dispatcher_market_jobs(conn, save_context, false)?;

    let open_market_jobs = generation.status.open_total_jobs;
    let active_jobs =
        count_dispatcher_jobs_by_status(conn, DISPATCHER_ACTIVE_JOB_STATUSES, save_context, None)?;
    let open_offers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM dispatcher_offers WHERE status IN ('draft', 'sent', 'under_review', 'countered')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let accepted_contracts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM dispatcher_contracts WHERE status = 'active'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(DispatcherOverview {
        open_market_jobs,
        active_jobs,
        open_offers,
        accepted_contracts,
    })
}

pub fn dispatcher_get_generation_status(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationStatus, String> {
    generation::dispatcher_get_generation_status(conn, save_context)
}

pub fn dispatcher_generate_jobs(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationStatus, String> {
    generation::dispatcher_generate_jobs(conn, save_context)
}

pub fn dispatcher_generate_universal_jobs(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
    force: bool,
    config: Option<DispatcherGenerationConfigInput>,
) -> Result<DispatcherGenerationStatus, String> {
    generation::dispatcher_generate_universal_jobs(conn, save_context, force, config)
}

pub fn dispatcher_cleanup_expired_jobs(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationStatus, String> {
    generation::dispatcher_cleanup_expired_jobs(conn, save_context)
}

pub fn dispatcher_restore_jobs_for_last_quicksave(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationStatus, String> {
    generation::dispatcher_restore_jobs_for_last_quicksave(conn, save_context)
}

pub fn dispatcher_background_tick(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherGenerationRunResult, String> {
    generation::dispatcher_background_tick(conn, save_context)
}

pub fn dispatcher_get_open_jobs(
    conn: &Connection,
    filter: Option<DispatcherJobFilter>,
    save_context: &DispatcherSaveContext,
) -> Result<Vec<DispatcherMarketJob>, String> {
    repo::dispatcher_get_market_jobs(conn, filter, save_context)
}

pub fn dispatcher_get_jobs_by_save_context(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
    status: Option<String>,
) -> Result<DispatcherJobsBySaveContextResponse, String> {
    repo::dispatcher_get_jobs_by_save_context(conn, save_context, status)
}

pub fn dispatcher_get_jobs_for_active_save(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobsBySaveContextResponse, String> {
    repo::dispatcher_get_jobs_for_active_save(conn, save_context)
}

pub fn dispatcher_link_job_to_save_context(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    repo::dispatcher_assign_job_to_active_save(conn, job_id, save_context)
}

pub fn dispatcher_assign_job_to_active_save(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    repo::dispatcher_assign_job_to_active_save(conn, job_id, save_context)
}

pub fn dispatcher_accept_generated_job(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    prepare_dispatcher_system(conn)?;
    let row = load_dispatcher_job_by_id(conn, job_id, save_context)?
        .ok_or_else(|| format!("dispatcher_job_not_found:{job_id}"))?;
    if row.source_type != "generated" {
        return Err("dispatcher_job_not_generated".to_string());
    }
    dispatcher_accept_job(conn, job_id, save_context)
}

pub fn dispatcher_mark_job_synced_to_ets2(
    conn: &Connection,
    job_id: &str,
    route_reference: Option<String>,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    prepare_dispatcher_system(conn)?;
    let row = load_dispatcher_job_by_id(conn, job_id, save_context)?
        .ok_or_else(|| format!("dispatcher_job_not_found:{job_id}"))?;
    let route_reference = route_reference
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(row.route_reference.clone())
        .unwrap_or_else(|| {
            build_dispatcher_route_reference(
                &row.company_id,
                &row.origin_country,
                &row.origin_city,
                &row.destination_country,
                &row.destination_city,
                &row.job_type,
            )
        });
    let now = Utc::now().to_rfc3339();

    conn.execute(
        r#"
        UPDATE dispatcher_jobs
        SET status = 'injected',
            route_reference = ?2,
            ets2_job_link_status = 'synced_to_ets2',
            last_error_code = NULL,
            last_error_message = NULL,
            updated_at_utc = ?3
        WHERE id = ?1
        "#,
        params![job_id, route_reference, now],
    )
    .map_err(|e| e.to_string())?;

    dispatcher_get_job_details(conn, job_id, save_context)
}

pub fn dispatcher_get_market_jobs(
    conn: &Connection,
    filter: Option<DispatcherJobFilter>,
    save_context: &DispatcherSaveContext,
) -> Result<Vec<DispatcherMarketJob>, String> {
    repo::dispatcher_get_market_jobs(conn, filter, save_context)
}

pub fn dispatcher_get_job_details(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    repo::dispatcher_get_job_details(conn, job_id, save_context)
}

pub fn dispatcher_get_job_by_id(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    repo::dispatcher_get_job_by_id(conn, job_id, save_context)
}

pub fn dispatcher_accept_job(
    conn: &Connection,
    job_id: &str,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    repo::dispatcher_accept_job(conn, job_id, save_context)
}

pub fn dispatcher_get_active_jobs(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<Vec<DispatcherMarketJob>, String> {
    repo::dispatcher_get_active_jobs(conn, save_context)
}

pub fn dispatcher_get_job_history(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<DispatcherHistoryResponse, String> {
    repo::dispatcher_get_job_history(conn, save_context)
}

pub fn dispatcher_get_company_contacts(
    conn: &Connection,
    save_context: &DispatcherSaveContext,
) -> Result<Vec<DispatcherCompanyContact>, String> {
    prepare_dispatcher_system(conn)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                p.company_id,
                COALESCE(p.company_name, p.company_id),
                p.payment_tier,
                p.payment_multiplier,
                COALESCE(r.reputation, 500),
                p.home_country_code,
                p.cargo_focus
            FROM company_payment_profiles p
            LEFT JOIN company_reputation r ON r.company_id = p.company_id
            ORDER BY r.reputation DESC, p.payment_multiplier DESC
            LIMIT 80
            "#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut contacts = Vec::new();
    for (
        company_id,
        company_name,
        tier_raw,
        payment_multiplier,
        reputation,
        home_country,
        cargo_focus,
    ) in rows
    {
        let tier = payment_tier_from_dispatcher_str(&tier_raw);
        let customer_multiplier =
            compensation_service::customer_multiplier(tier, payment_multiplier);
        let reputation_u16 = reputation.clamp(0, 1000) as u16;
        let reputation_multiplier = compensation_service::reputation_multiplier(reputation_u16);
        let country_multiplier = if let Some(code) = home_country.as_deref() {
            compensation_service::load_country_payment_level(conn, code)
                .map(|level| level.payment_multiplier)
                .unwrap_or(1.0)
        } else {
            1.0
        };
        let completed_jobs =
            count_dispatcher_company_jobs(conn, &company_id, &["completed"], save_context)?;
        let failed_jobs = count_dispatcher_company_jobs(
            conn,
            &company_id,
            &["problematic", "cancelled"],
            save_context,
        )?;
        let accepted_offers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM dispatcher_offers WHERE company_id = ?1 AND status = 'accepted'",
                [company_id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let rejected_offers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM dispatcher_offers WHERE company_id = ?1 AND status IN ('rejected', 'expired')",
                [company_id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let base = (completed_jobs + failed_jobs + accepted_offers + rejected_offers).max(1) as f64;
        let success_rate = ((completed_jobs + accepted_offers) as f64 / base).clamp(0.0, 1.0);
        contacts.push(DispatcherCompanyContact {
            company_id,
            company_name,
            payment_tier: tier_raw,
            payment_multiplier,
            customer_multiplier,
            reputation: reputation_u16,
            reputation_multiplier,
            home_country_code: home_country,
            country_multiplier,
            cargo_focus,
            completed_jobs,
            failed_jobs,
            accepted_offers,
            rejected_offers,
            success_rate,
        });
    }
    Ok(contacts)
}

pub fn dispatcher_get_offers(conn: &Connection) -> Result<Vec<DispatcherOffer>, String> {
    prepare_dispatcher_system(conn)?;
    expire_dispatcher_offers(conn)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                company_id,
                company_name,
                user_id,
                offer_type,
                requested_job_type,
                requested_cargo_type,
                requested_region,
                proposed_rate_per_km,
                note,
                equipment_type,
                contract_scope,
                status,
                counter_rate_per_km,
                final_rate_per_km,
                response_reason,
                linked_job_id,
                created_at_utc,
                updated_at_utc,
                expires_at_utc
            FROM dispatcher_offers
            ORDER BY created_at_utc DESC
            LIMIT 200
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], map_dispatcher_offer_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn dispatcher_create_offer(
    conn: &Connection,
    input: DispatcherCreateOfferInput,
) -> Result<DispatcherOffer, String> {
    prepare_dispatcher_system(conn)?;
    let company_id = input.company_id.trim();
    if company_id.is_empty() {
        return Err("dispatcher_offer_company_required".to_string());
    }

    let profile = compensation_service::load_company_payment_profile(conn, company_id, None)?;
    let company_name = profile
        .company_name
        .clone()
        .unwrap_or_else(|| company_id.to_string());
    let reputation = compensation_service::load_company_reputation(conn, company_id)?;
    let requested_job_type = normalize_dispatcher_job_type(&input.requested_job_type);
    let requested_cargo_type = input
        .requested_cargo_type
        .as_deref()
        .map(normalize_dispatcher_cargo_type)
        .unwrap_or_else(|| "standard".to_string());
    let equipment_type = input
        .equipment_type
        .as_deref()
        .map(normalize_dispatcher_equipment_type)
        .unwrap_or_else(|| "own_truck".to_string());
    let requested_rate = input.proposed_rate_per_km.unwrap_or(1.0);

    let target_pricing = compensation_service::calculate_job_compensation(
        conn,
        &JobCompensationInput {
            company_id: company_id.to_string(),
            company_name: Some(company_name.clone()),
            distance_km: 500.0,
            base_rate_type: base_rate_type_for_dispatcher_job(&requested_job_type),
            equipment_type: equipment_type_for_dispatcher_job(&equipment_type),
            cargo_type: cargo_type_from_dispatcher_string(&requested_cargo_type),
            urgency: Urgency::Normal,
            origin_country_code: "DE".to_string(),
            destination_country_code: "DE".to_string(),
            market_seed: Utc::now().timestamp_millis().unsigned_abs(),
        },
    )?;
    let target_rate = target_pricing.final_rate_per_km.max(0.55);

    let success_rate = dispatcher_success_rate_for_company(conn, company_id)?;
    let score = dispatcher_offer_acceptance_score(
        requested_rate,
        target_rate,
        profile.payment_tier,
        reputation.reputation,
        success_rate,
    );

    let now = Utc::now();
    let default_expiry = Some((now + Duration::hours(36)).to_rfc3339());
    let (status, counter_rate, final_rate, response_reason, expires) = if score >= 72.0 {
        (
            "accepted".to_string(),
            None,
            Some(requested_rate),
            Some("offer_aligned_with_profile".to_string()),
            None,
        )
    } else if score >= 48.0 {
        (
            "countered".to_string(),
            Some(((requested_rate + target_rate) / 2.0 * 100.0).round() / 100.0),
            None,
            Some("counter_offer_requested".to_string()),
            default_expiry,
        )
    } else if score >= 38.0 {
        (
            "under_review".to_string(),
            None,
            None,
            Some("manual_review_pending".to_string()),
            Some((now + Duration::hours(18)).to_rfc3339()),
        )
    } else {
        (
            "rejected".to_string(),
            None,
            None,
            Some("rate_above_company_band".to_string()),
            None,
        )
    };

    let offer_id = format!(
        "offer-{}-{}",
        now.timestamp_millis(),
        company_id.bytes().fold(0_u64, |acc, byte| acc
            .wrapping_mul(131)
            .wrapping_add(byte as u64))
            % 997
    );
    let user_id = input
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("local-player")
        .to_string();
    let note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let created = now.to_rfc3339();

    conn.execute(
        r#"
        INSERT INTO dispatcher_offers (
            id,
            company_id,
            company_name,
            user_id,
            offer_type,
            requested_job_type,
            requested_cargo_type,
            requested_region,
            proposed_rate_per_km,
            note,
            equipment_type,
            contract_scope,
            status,
            counter_rate_per_km,
            final_rate_per_km,
            response_reason,
            linked_job_id,
            created_at_utc,
            updated_at_utc,
            expires_at_utc
        )
        VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, NULL, ?17, ?17, ?18
        )
        "#,
        params![
            offer_id,
            company_id,
            company_name,
            user_id,
            normalize_dispatcher_offer_type(&input.offer_type),
            requested_job_type,
            Some(requested_cargo_type),
            input.requested_region,
            input.proposed_rate_per_km,
            note,
            Some(equipment_type),
            input.contract_scope,
            status,
            counter_rate,
            final_rate,
            response_reason,
            created,
            expires,
        ],
    )
    .map_err(|e| e.to_string())?;

    let mut created_offer = load_dispatcher_offer_by_id(conn, &offer_id)?
        .ok_or_else(|| "dispatcher_offer_not_found_after_insert".to_string())?;
    if created_offer.status == "accepted" {
        maybe_create_dispatcher_contract(conn, &created_offer)?;
        created_offer = load_dispatcher_offer_by_id(conn, &offer_id)?
            .ok_or_else(|| "dispatcher_offer_not_found_after_insert".to_string())?;
    }

    Ok(created_offer)
}

pub fn dispatcher_cancel_offer(
    conn: &Connection,
    offer_id: &str,
) -> Result<DispatcherOffer, String> {
    prepare_dispatcher_system(conn)?;
    let offer = load_dispatcher_offer_by_id(conn, offer_id)?
        .ok_or_else(|| format!("dispatcher_offer_not_found:{offer_id}"))?;
    if !matches!(
        offer.status.as_str(),
        "draft" | "sent" | "under_review" | "countered"
    ) {
        return Err("dispatcher_offer_not_cancellable".to_string());
    }

    conn.execute(
        "UPDATE dispatcher_offers SET status = 'cancelled', updated_at_utc = ?2 WHERE id = ?1",
        params![offer_id, Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;

    load_dispatcher_offer_by_id(conn, offer_id)?
        .ok_or_else(|| format!("dispatcher_offer_not_found:{offer_id}"))
}

pub fn dispatcher_respond_to_counter(
    conn: &Connection,
    input: DispatcherRespondToCounterInput,
) -> Result<DispatcherOffer, String> {
    prepare_dispatcher_system(conn)?;
    let offer = load_dispatcher_offer_by_id(conn, &input.offer_id)?
        .ok_or_else(|| format!("dispatcher_offer_not_found:{}", input.offer_id))?;
    if offer.status != "countered" {
        return Err("dispatcher_offer_not_countered".to_string());
    }

    let now = Utc::now().to_rfc3339();
    if input.accept_counter {
        conn.execute(
            "UPDATE dispatcher_offers SET status = 'accepted', final_rate_per_km = counter_rate_per_km, response_reason = 'counter_accepted', updated_at_utc = ?2, expires_at_utc = NULL WHERE id = ?1",
            params![input.offer_id, now],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE dispatcher_offers SET status = 'rejected', response_reason = 'counter_declined', updated_at_utc = ?2, expires_at_utc = NULL WHERE id = ?1",
            params![input.offer_id, now],
        )
        .map_err(|e| e.to_string())?;
    }

    let updated = load_dispatcher_offer_by_id(conn, &input.offer_id)?
        .ok_or_else(|| format!("dispatcher_offer_not_found:{}", input.offer_id))?;
    if updated.status == "accepted" {
        maybe_create_dispatcher_contract(conn, &updated)?;
    }

    load_dispatcher_offer_by_id(conn, &input.offer_id)?
        .ok_or_else(|| format!("dispatcher_offer_not_found:{}", input.offer_id))
}

fn map_dispatcher_offer_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DispatcherOffer> {
    Ok(DispatcherOffer {
        id: row.get(0)?,
        company_id: row.get(1)?,
        company_name: row.get(2)?,
        user_id: row.get(3)?,
        offer_type: row.get(4)?,
        requested_job_type: row.get(5)?,
        requested_cargo_type: row.get(6)?,
        requested_region: row.get(7)?,
        proposed_rate_per_km: row.get(8)?,
        note: row.get(9)?,
        equipment_type: row.get(10)?,
        contract_scope: row.get(11)?,
        status: row.get(12)?,
        counter_rate_per_km: row.get(13)?,
        final_rate_per_km: row.get(14)?,
        response_reason: row.get(15)?,
        linked_job_id: row.get(16)?,
        created_at_utc: row.get(17)?,
        updated_at_utc: row.get(18)?,
        expires_at_utc: row.get(19)?,
    })
}

fn load_dispatcher_offer_by_id(
    conn: &Connection,
    offer_id: &str,
) -> Result<Option<DispatcherOffer>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, company_id, company_name, user_id, offer_type, requested_job_type,
                requested_cargo_type, requested_region, proposed_rate_per_km, note,
                equipment_type, contract_scope, status, counter_rate_per_km, final_rate_per_km,
                response_reason, linked_job_id, created_at_utc, updated_at_utc, expires_at_utc
            FROM dispatcher_offers
            WHERE id = ?1
            "#,
        )
        .map_err(|e| e.to_string())?;
    stmt.query_row([offer_id], map_dispatcher_offer_row)
        .optional()
        .map_err(|e| e.to_string())
}

fn expire_dispatcher_offers(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE dispatcher_offers SET status = 'expired', updated_at_utc = ?1 WHERE status IN ('draft', 'sent', 'under_review', 'countered') AND expires_at_utc IS NOT NULL AND expires_at_utc < ?1",
        [Utc::now().to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn maybe_create_dispatcher_contract(
    conn: &Connection,
    offer: &DispatcherOffer,
) -> Result<(), String> {
    let scope = offer
        .contract_scope
        .as_deref()
        .unwrap_or("one_time")
        .to_ascii_lowercase();
    if !matches!(scope.as_str(), "recurring" | "contract" | "long_term") {
        return Ok(());
    }
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM dispatcher_contracts WHERE id = ?1",
            [format!("contract-{}", offer.id)],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if existing.is_some() {
        return Ok(());
    }
    let rate_modifier = offer
        .final_rate_per_km
        .unwrap_or(offer.proposed_rate_per_km.unwrap_or(1.0));
    conn.execute(
        r#"
        INSERT INTO dispatcher_contracts (
            id,
            company_id,
            user_id,
            contract_type,
            agreed_rate_modifier,
            preferred_cargo_type,
            region_scope,
            active_from_utc,
            active_until_utc,
            status
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, 'active')
        "#,
        params![
            format!("contract-{}", offer.id),
            offer.company_id,
            offer.user_id,
            scope,
            rate_modifier,
            offer.requested_cargo_type,
            offer.requested_region,
            Utc::now().to_rfc3339(),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn dispatcher_equipment_ok(conn: &Connection, required: &str) -> Result<bool, String> {
    let trucks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM fleet_assets WHERE kind = 'truck' AND status IN ('player', 'assigned', 'ready')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let trailers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM fleet_assets WHERE kind = 'trailer'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let required = normalize_dispatcher_equipment_type(required);
    Ok(match required.as_str() {
        "quick_job" => true,
        "own_truck" => trucks > 0,
        "own_truck_trailer" => trucks > 0 && trailers > 0,
        _ => trucks > 0,
    })
}

fn dispatcher_reputation_requirement_for(difficulty_level: &str) -> u16 {
    match difficulty_level.to_ascii_lowercase().as_str() {
        "expert" => 650,
        "hard" => 420,
        "normal" => 250,
        _ => 120,
    }
}

fn dispatcher_success_rate_for_company(conn: &Connection, company_id: &str) -> Result<f64, String> {
    let completed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM dispatcher_jobs WHERE company_id = ?1 AND status = 'completed'",
            [company_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let failed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM dispatcher_jobs WHERE company_id = ?1 AND status IN ('problematic', 'cancelled', 'rejected')",
            [company_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let base = (completed + failed).max(1) as f64;
    Ok((completed as f64 / base).clamp(0.0, 1.0))
}

fn dispatcher_offer_acceptance_score(
    requested_rate: f64,
    target_rate: f64,
    payment_tier: CompanyPaymentTier,
    reputation: u16,
    success_rate: f64,
) -> f64 {
    let ratio = if target_rate <= 0.0 {
        1.0
    } else {
        requested_rate / target_rate
    };
    let rate_score = if ratio <= 0.92 {
        93.0
    } else if ratio <= 1.0 {
        82.0
    } else if ratio <= 1.08 {
        68.0
    } else if ratio <= 1.16 {
        46.0
    } else {
        20.0
    };
    let tier_bonus = match payment_tier {
        CompanyPaymentTier::Budget => -8.0,
        CompanyPaymentTier::Standard => 0.0,
        CompanyPaymentTier::Good => 4.0,
        CompanyPaymentTier::Premium => 8.0,
        CompanyPaymentTier::Elite => 12.0,
    };
    let reputation_score = (reputation as f64 / 10.0).clamp(0.0, 100.0);
    (rate_score * 0.5 + reputation_score * 0.25 + success_rate * 100.0 * 0.25 + tier_bonus)
        .clamp(0.0, 100.0)
}

fn base_rate_type_for_dispatcher_job(job_type: &str) -> BaseRateType {
    match normalize_dispatcher_job_type(job_type).as_str() {
        "quick_job" => BaseRateType::QuickJob,
        "cargo_market" | "premium_special" => BaseRateType::OwnTruckOwnTrailer,
        _ => BaseRateType::OwnTruck,
    }
}

fn equipment_type_for_dispatcher_job(equipment_type: &str) -> EquipmentType {
    match normalize_dispatcher_equipment_type(equipment_type).as_str() {
        "quick_job" => EquipmentType::QuickJob,
        "own_truck_trailer" => EquipmentType::OwnTruckOwnTrailer,
        _ => EquipmentType::OwnTruck,
    }
}

fn dispatcher_job_type_modifier(job_type: &str) -> f64 {
    match normalize_dispatcher_job_type(job_type).as_str() {
        "quick_job" => 0.92,
        "freight_market" => 1.0,
        "cargo_market" => 1.08,
        "company_contract" => 1.12,
        "premium_special" => 1.2,
        _ => 1.0,
    }
}

fn dispatcher_difficulty_modifier(difficulty: &str) -> f64 {
    match difficulty.to_ascii_lowercase().as_str() {
        "easy" => 0.95,
        "normal" => 1.0,
        "hard" => 1.07,
        "expert" => 1.14,
        _ => 1.0,
    }
}

fn payment_tier_to_db(value: CompanyPaymentTier) -> &'static str {
    match value {
        CompanyPaymentTier::Budget => "budget",
        CompanyPaymentTier::Standard => "standard",
        CompanyPaymentTier::Good => "good",
        CompanyPaymentTier::Premium => "premium",
        CompanyPaymentTier::Elite => "elite",
    }
}

fn payment_tier_from_dispatcher_str(value: &str) -> CompanyPaymentTier {
    match value.trim().to_ascii_lowercase().as_str() {
        "budget" => CompanyPaymentTier::Budget,
        "good" => CompanyPaymentTier::Good,
        "premium" => CompanyPaymentTier::Premium,
        "elite" => CompanyPaymentTier::Elite,
        _ => CompanyPaymentTier::Standard,
    }
}

fn cargo_type_to_db(value: CargoType) -> &'static str {
    match value {
        CargoType::Standard => "standard",
        CargoType::Fragile => "fragile",
        CargoType::Refrigerated => "refrigerated",
        CargoType::Valuable => "valuable",
        CargoType::Hazardous => "hazardous",
        CargoType::Oversize => "oversize",
    }
}

fn cargo_type_from_dispatcher_string(value: &str) -> CargoType {
    match normalize_dispatcher_cargo_type(value).as_str() {
        "fragile" => CargoType::Fragile,
        "refrigerated" => CargoType::Refrigerated,
        "valuable" => CargoType::Valuable,
        "hazardous" => CargoType::Hazardous,
        "oversize" => CargoType::Oversize,
        _ => CargoType::Standard,
    }
}

fn normalize_dispatcher_offer_type(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        "quote_request".to_string()
    } else {
        normalized
    }
}

fn normalize_dispatcher_job_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "quick" | "quick_job" => "quick_job".to_string(),
        "freight" | "freight_market" => "freight_market".to_string(),
        "cargo" | "cargo_market" => "cargo_market".to_string(),
        "company" | "company_contract" | "direct_contract" => "company_contract".to_string(),
        "premium" | "premium_special" | "special" => "premium_special".to_string(),
        other if !other.is_empty() => other.to_string(),
        _ => "freight_market".to_string(),
    }
}

fn normalize_dispatcher_cargo_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "fragile" => "fragile".to_string(),
        "refrigerated" => "refrigerated".to_string(),
        "valuable" => "valuable".to_string(),
        "hazardous" => "hazardous".to_string(),
        "oversize" => "oversize".to_string(),
        _ => "standard".to_string(),
    }
}

fn normalize_dispatcher_equipment_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "quick" | "quick_job" => "quick_job".to_string(),
        "own_truck" | "truck" => "own_truck".to_string(),
        "own_truck_trailer" | "own_truck_own_trailer" | "trailer" => {
            "own_truck_trailer".to_string()
        }
        _ => "own_truck".to_string(),
    }
}
