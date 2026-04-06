use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

use crate::features::economy::compensation_models::{
    BaseRateType, CargoType, CompanyPaymentProfile, CompanyPaymentTier, CompanyReputationOutcome,
    CompanyReputationState, CompanyCompensationCondition, CountryPaymentLevel, EquipmentType,
    JobCompensationInput, JobCompensationResult, UpsertCompanyPaymentProfileInput, Urgency,
};

const DEFAULT_COMPANY_REPUTATION: u16 = 500;
const MIN_REPUTATION_MULTIPLIER: f64 = 0.92;
const MAX_REPUTATION_MULTIPLIER: f64 = 1.28;

pub fn calculate_job_compensation(
    conn: &Connection,
    input: &JobCompensationInput,
) -> Result<JobCompensationResult, String> {
    if input.company_id.trim().is_empty() {
        return Err("company_id_missing".to_string());
    }
    if input.distance_km <= 0.0 {
        return Err("distance_km_must_be_positive".to_string());
    }

    let origin_country_code = normalize_country_code(&input.origin_country_code);
    let destination_country_code = normalize_country_code(&input.destination_country_code);

    let company_payment = load_company_payment_profile(
        conn,
        &input.company_id,
        input.company_name.as_deref(),
    )?;
    let company_reputation = load_company_reputation(conn, &input.company_id)?;

    let base_rate_per_km = base_rate_per_km(input.base_rate_type);
    let customer_multiplier =
        customer_multiplier(company_payment.payment_tier, company_payment.payment_multiplier);
    let country_multiplier =
        resolve_country_multiplier(conn, &origin_country_code, &destination_country_code)?;
    let company_reputation_multiplier = reputation_multiplier(company_reputation.reputation);
    let cargo_multiplier = cargo_multiplier(input.cargo_type);
    let urgency_multiplier = urgency_multiplier(input.urgency);
    let equipment_multiplier = equipment_multiplier(input.equipment_type);
    let market_variation = market_variation(input.market_seed, &input.company_id);

    let final_rate_per_km = base_rate_per_km
        * customer_multiplier
        * country_multiplier
        * company_reputation_multiplier
        * cargo_multiplier
        * urgency_multiplier
        * equipment_multiplier
        * market_variation;

    let final_price = (input.distance_km * final_rate_per_km).max(0.0).round() as i64;

    Ok(JobCompensationResult {
        distance_km: input.distance_km,
        company_id: input.company_id.clone(),
        origin_country_code,
        destination_country_code,
        company_reputation: company_reputation.reputation,
        company_payment_tier: company_payment.payment_tier,
        company_payment_multiplier: company_payment.payment_multiplier,
        base_rate_per_km,
        customer_multiplier,
        country_multiplier,
        equipment_multiplier,
        company_reputation_multiplier,
        cargo_multiplier,
        urgency_multiplier,
        market_variation,
        final_rate_per_km,
        final_price,
    })
}

pub fn load_company_payment_profile(
    conn: &Connection,
    company_id: &str,
    company_name_hint: Option<&str>,
) -> Result<CompanyPaymentProfile, String> {
    ensure_company_payment_profile_row(conn, company_id, company_name_hint)?;

    conn.query_row(
        r#"
        SELECT
            company_id,
            company_name,
            payment_tier,
            payment_multiplier,
            home_country_code,
            cargo_focus,
            updated_at_utc
        FROM company_payment_profiles
        WHERE company_id = ?1
        "#,
        [company_id],
        |row| {
            let payment_tier_raw: String = row.get(2)?;
            let payment_multiplier: f64 = row.get(3)?;
            let home_country_code = row.get::<_, Option<String>>(4)?.map(|code| normalize_country_code(&code));
            Ok(CompanyPaymentProfile {
                company_id: row.get(0)?,
                company_name: normalize_optional_text(row.get::<_, Option<String>>(1)?),
                payment_tier: payment_tier_from_db(&payment_tier_raw),
                payment_multiplier: payment_multiplier.clamp(0.80, 1.40),
                home_country_code,
                cargo_focus: normalize_optional_text(row.get::<_, Option<String>>(5)?),
                updated_at_utc: row.get(6)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn upsert_company_payment_profile(
    conn: &Connection,
    input: &UpsertCompanyPaymentProfileInput,
) -> Result<(), String> {
    let company_id = input.company_id.trim();
    if company_id.is_empty() {
        return Err("company_id_missing".to_string());
    }

    let now = Utc::now().to_rfc3339();
    let company_name = normalize_optional_text(input.company_name.clone());
    let home_country_code = input
        .home_country_code
        .as_ref()
        .map(|value| normalize_country_code(value));
    let cargo_focus = normalize_optional_text(input.cargo_focus.clone());
    let payment_multiplier = input.payment_multiplier.clamp(0.80, 1.40);

    conn.execute(
        r#"
        INSERT INTO company_payment_profiles (
            company_id,
            company_name,
            payment_tier,
            payment_multiplier,
            home_country_code,
            cargo_focus,
            updated_at_utc
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(company_id) DO UPDATE SET
            company_name = COALESCE(excluded.company_name, company_payment_profiles.company_name),
            payment_tier = excluded.payment_tier,
            payment_multiplier = excluded.payment_multiplier,
            home_country_code = COALESCE(excluded.home_country_code, company_payment_profiles.home_country_code),
            cargo_focus = COALESCE(excluded.cargo_focus, company_payment_profiles.cargo_focus),
            updated_at_utc = excluded.updated_at_utc
        "#,
        params![
            company_id,
            company_name,
            payment_tier_to_db(input.payment_tier),
            payment_multiplier,
            home_country_code,
            cargo_focus,
            now
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn ensure_company_payment_profile_row(
    conn: &Connection,
    company_id: &str,
    company_name_hint: Option<&str>,
) -> Result<(), String> {
    let company_id = company_id.trim();
    if company_id.is_empty() {
        return Err("company_id_missing".to_string());
    }

    let existing = conn
        .query_row(
            "SELECT company_id FROM company_payment_profiles WHERE company_id = ?1",
            [company_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let normalized_name = normalize_optional_text(company_name_hint.map(|value| value.to_string()));
    if existing.is_none() {
        conn.execute(
            r#"
            INSERT INTO company_payment_profiles (
                company_id,
                company_name,
                payment_tier,
                payment_multiplier,
                home_country_code,
                cargo_focus,
                updated_at_utc
            )
            VALUES (?1, ?2, 'standard', 1.0, NULL, NULL, ?3)
            "#,
            params![company_id, normalized_name, Utc::now().to_rfc3339()],
        )
        .map_err(|e| e.to_string())?;
        return Ok(());
    }

    if let Some(company_name) = normalized_name {
        conn.execute(
            r#"
            UPDATE company_payment_profiles
            SET company_name = ?2, updated_at_utc = ?3
            WHERE company_id = ?1
            "#,
            params![company_id, company_name, Utc::now().to_rfc3339()],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn load_country_payment_level(
    conn: &Connection,
    country_code: &str,
) -> Result<CountryPaymentLevel, String> {
    let code = normalize_country_code(country_code);
    let row = conn
        .query_row(
            r#"
            SELECT country_code, country_name, payment_multiplier
            FROM country_payment_levels
            WHERE country_code = ?1
            "#,
            [code.as_str()],
            |row| {
                Ok(CountryPaymentLevel {
                    country_code: row.get(0)?,
                    country_name: row.get(1)?,
                    payment_multiplier: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    Ok(row.unwrap_or(CountryPaymentLevel {
        country_code: code.clone(),
        country_name: code,
        payment_multiplier: 1.0,
    }))
}

pub fn resolve_country_multiplier(
    conn: &Connection,
    origin_country_code: &str,
    destination_country_code: &str,
) -> Result<f64, String> {
    let origin = load_country_payment_level(conn, origin_country_code)?;
    let destination = load_country_payment_level(conn, destination_country_code)?;
    Ok(((origin.payment_multiplier + destination.payment_multiplier) / 2.0).clamp(0.80, 1.30))
}

pub fn list_company_compensation_conditions(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<CompanyCompensationCondition>, String> {
    #[derive(Debug)]
    struct RawCompanyCondition {
        company_id: String,
        company_name: Option<String>,
        payment_tier_raw: String,
        payment_multiplier: f64,
        home_country_code: Option<String>,
        cargo_focus: Option<String>,
        updated_at_utc: String,
        reputation: i64,
    }

    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                p.company_id,
                p.company_name,
                p.payment_tier,
                p.payment_multiplier,
                p.home_country_code,
                p.cargo_focus,
                p.updated_at_utc,
                COALESCE(r.reputation, ?2) AS reputation
            FROM company_payment_profiles p
            LEFT JOIN company_reputation r ON r.company_id = p.company_id
            ORDER BY
                CASE p.payment_tier
                    WHEN 'elite' THEN 5
                    WHEN 'premium' THEN 4
                    WHEN 'good' THEN 3
                    WHEN 'standard' THEN 2
                    ELSE 1
                END DESC,
                p.payment_multiplier DESC,
                reputation DESC,
                p.company_id ASC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let raw_rows = stmt
        .query_map(params![limit as i64, DEFAULT_COMPANY_REPUTATION as i64], |row| {
            Ok(RawCompanyCondition {
                company_id: row.get(0)?,
                company_name: row.get(1)?,
                payment_tier_raw: row.get(2)?,
                payment_multiplier: row.get(3)?,
                home_country_code: row.get(4)?,
                cargo_focus: row.get(5)?,
                updated_at_utc: row.get(6)?,
                reputation: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let raw_rows = raw_rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut conditions = Vec::with_capacity(raw_rows.len());
    for raw in raw_rows {
        let payment_tier = payment_tier_from_db(&raw.payment_tier_raw);
        let payment_multiplier = raw.payment_multiplier.clamp(0.80, 1.40);
        let reputation = raw.reputation.clamp(0, 1000) as u16;
        let home_country_code = raw
            .home_country_code
            .as_ref()
            .map(|value| normalize_country_code(value));
        let home_country_multiplier = if let Some(code) = home_country_code.as_deref() {
            load_country_payment_level(conn, code)?.payment_multiplier
        } else {
            1.0
        };
        let customer_multiplier = customer_multiplier(payment_tier, payment_multiplier);
        let reputation_multiplier = reputation_multiplier(reputation);

        conditions.push(CompanyCompensationCondition {
            company_id: raw.company_id,
            company_name: normalize_optional_text(raw.company_name)
                .unwrap_or_else(|| "Unknown Company".to_string()),
            payment_tier,
            payment_multiplier,
            customer_multiplier,
            reputation,
            reputation_multiplier,
            home_country_code,
            home_country_multiplier,
            cargo_focus: normalize_optional_text(raw.cargo_focus),
            effective_multiplier: (customer_multiplier * reputation_multiplier * home_country_multiplier)
                .clamp(0.70, 2.20),
            updated_at_utc: raw.updated_at_utc,
        });
    }

    Ok(conditions)
}

pub fn list_country_payment_levels(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<CountryPaymentLevel>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT country_code, country_name, payment_multiplier
            FROM country_payment_levels
            ORDER BY payment_multiplier DESC, country_name ASC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(CountryPaymentLevel {
                country_code: row.get(0)?,
                country_name: row.get(1)?,
                payment_multiplier: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn normalize_country_code(value: &str) -> String {
    let code = value.trim().to_ascii_uppercase();
    match code.as_str() {
        "" => "DE".to_string(),
        "GB" => "UK".to_string(),
        "CZE" => "CZ".to_string(),
        "SVK" => "SK".to_string(),
        "AUT" => "AT".to_string(),
        "DEU" => "DE".to_string(),
        "CHE" => "CH".to_string(),
        "NOR" => "NO".to_string(),
        "SWE" => "SE".to_string(),
        "DNK" => "DK".to_string(),
        "MKD" => "MK".to_string(),
        _ => code,
    }
}

pub fn infer_country_code_from_city(city: &str) -> Option<&'static str> {
    let city = city.trim().to_ascii_lowercase();
    match city.as_str() {
        "hamburg" | "berlin" | "munich" | "muenchen" | "frankfurt" | "dresden" | "leipzig"
        | "kiel" => Some("DE"),
        "prague" | "praha" | "brno" => Some("CZ"),
        "vienna" | "wien" => Some("AT"),
        "warsaw" | "warszawa" => Some("PL"),
        "rotterdam" => Some("NL"),
        "lyon" => Some("FR"),
        "oslo" => Some("NO"),
        "genoa" | "genova" => Some("IT"),
        "brussels" | "bruxelles" | "brussel" => Some("BE"),
        "lisbon" | "lisboa" => Some("PT"),
        "madrid" | "barcelona" => Some("ES"),
        "london" | "manchester" | "birmingham" => Some("UK"),
        "stockholm" | "goteborg" | "gothenburg" => Some("SE"),
        "copenhagen" | "kobenhavn" => Some("DK"),
        "zurich" | "zuerich" | "geneva" | "genf" => Some("CH"),
        "bratislava" => Some("SK"),
        "budapest" => Some("HU"),
        "zagreb" => Some("HR"),
        "bucharest" | "bucuresti" => Some("RO"),
        "sofia" => Some("BG"),
        "belgrade" | "beograd" => Some("RS"),
        "tirana" => Some("AL"),
        "skopje" => Some("MK"),
        "podgorica" => Some("ME"),
        _ => None,
    }
}

pub fn load_company_reputation(
    conn: &Connection,
    company_id: &str,
) -> Result<CompanyReputationState, String> {
    ensure_company_reputation_row(conn, company_id)?;

    conn.query_row(
        r#"
        SELECT
            company_id,
            reputation,
            reliability_streak,
            completed_jobs,
            late_jobs,
            damage_incidents,
            canceled_jobs,
            updated_at_utc
        FROM company_reputation
        WHERE company_id = ?1
        "#,
        [company_id],
        |row| {
            let reputation: i64 = row.get(1)?;
            let reliability_streak: i64 = row.get(2)?;
            let completed_jobs: i64 = row.get(3)?;
            let late_jobs: i64 = row.get(4)?;
            let damage_incidents: i64 = row.get(5)?;
            let canceled_jobs: i64 = row.get(6)?;

            Ok(CompanyReputationState {
                company_id: row.get(0)?,
                reputation: reputation.clamp(0, 1000) as u16,
                reliability_streak: reliability_streak.max(0) as u16,
                completed_jobs: completed_jobs.max(0) as u32,
                late_jobs: late_jobs.max(0) as u32,
                damage_incidents: damage_incidents.max(0) as u32,
                canceled_jobs: canceled_jobs.max(0) as u32,
                updated_at_utc: row.get(7)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn apply_company_reputation_outcome(
    conn: &Connection,
    company_id: &str,
    outcome: CompanyReputationOutcome,
) -> Result<CompanyReputationState, String> {
    let current = load_company_reputation(conn, company_id)?;
    let normalized_damage = outcome.damage_percent.clamp(0.0, 100.0);

    let mut delta = 0_i64;
    if outcome.completed {
        delta += 8;
    } else {
        delta -= 18;
    }

    if outcome.on_time {
        delta += 6;
    } else {
        delta -= 10;
    }

    if normalized_damage <= 1.0 {
        delta += 3;
    } else if normalized_damage > 4.0 && normalized_damage <= 10.0 {
        delta -= 6;
    } else if normalized_damage > 10.0 && normalized_damage <= 20.0 {
        delta -= 12;
    } else if normalized_damage > 20.0 {
        delta -= 22;
    }

    if outcome.canceled {
        delta -= 28;
    }

    let clean_reliable_delivery =
        outcome.completed && outcome.on_time && !outcome.canceled && normalized_damage <= 1.0;
    if clean_reliable_delivery {
        delta += ((current.reliability_streak / 5).min(5)) as i64;
    }

    let next_reputation = (current.reputation as i64 + delta).clamp(0, 1000) as u16;
    let next_reliability_streak = if clean_reliable_delivery {
        current.reliability_streak.saturating_add(1)
    } else {
        0
    };

    let next_completed_jobs = current
        .completed_jobs
        .saturating_add(if outcome.completed { 1 } else { 0 });
    let next_late_jobs = current
        .late_jobs
        .saturating_add(if outcome.on_time { 0 } else { 1 });
    let next_damage_incidents = current
        .damage_incidents
        .saturating_add(if normalized_damage > 1.0 { 1 } else { 0 });
    let next_canceled_jobs = current
        .canceled_jobs
        .saturating_add(if outcome.canceled { 1 } else { 0 });

    conn.execute(
        r#"
        UPDATE company_reputation
        SET
            reputation = ?2,
            reliability_streak = ?3,
            completed_jobs = ?4,
            late_jobs = ?5,
            damage_incidents = ?6,
            canceled_jobs = ?7,
            updated_at_utc = ?8
        WHERE company_id = ?1
        "#,
        params![
            company_id,
            next_reputation as i64,
            next_reliability_streak as i64,
            next_completed_jobs as i64,
            next_late_jobs as i64,
            next_damage_incidents as i64,
            next_canceled_jobs as i64,
            Utc::now().to_rfc3339()
        ],
    )
    .map_err(|e| e.to_string())?;

    load_company_reputation(conn, company_id)
}

pub fn ensure_company_reputation_row(conn: &Connection, company_id: &str) -> Result<(), String> {
    if company_id.trim().is_empty() {
        return Err("company_id_missing".to_string());
    }

    let existing = conn
        .query_row(
            "SELECT company_id FROM company_reputation WHERE company_id = ?1",
            [company_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if existing.is_none() {
        conn.execute(
            r#"
            INSERT INTO company_reputation (
                company_id,
                reputation,
                reliability_streak,
                completed_jobs,
                late_jobs,
                damage_incidents,
                canceled_jobs,
                updated_at_utc
            )
            VALUES (?1, ?2, 0, 0, 0, 0, 0, ?3)
            "#,
            params![
                company_id,
                DEFAULT_COMPANY_REPUTATION as i64,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn base_rate_per_km(rate_type: BaseRateType) -> f64 {
    match rate_type {
        BaseRateType::QuickJob => 0.72,
        BaseRateType::OwnTruck => 1.08,
        BaseRateType::OwnTruckOwnTrailer => 1.34,
    }
}

pub fn equipment_multiplier(equipment_type: EquipmentType) -> f64 {
    match equipment_type {
        EquipmentType::QuickJob => 0.98,
        EquipmentType::OwnTruck => 1.00,
        EquipmentType::OwnTruckOwnTrailer => 1.03,
    }
}

pub fn reputation_multiplier(reputation: u16) -> f64 {
    let normalized = (reputation as f64 / 1000.0).clamp(0.0, 1.0);
    MIN_REPUTATION_MULTIPLIER
        + (MAX_REPUTATION_MULTIPLIER - MIN_REPUTATION_MULTIPLIER) * normalized
}

pub fn customer_tier_multiplier(tier: CompanyPaymentTier) -> f64 {
    match tier {
        CompanyPaymentTier::Budget => 0.92,
        CompanyPaymentTier::Standard => 1.00,
        CompanyPaymentTier::Good => 1.08,
        CompanyPaymentTier::Premium => 1.18,
        CompanyPaymentTier::Elite => 1.30,
    }
}

pub fn customer_multiplier(tier: CompanyPaymentTier, payment_multiplier: f64) -> f64 {
    let normalized_direct = payment_multiplier.clamp(0.80, 1.40);
    (customer_tier_multiplier(tier) * normalized_direct).clamp(0.82, 1.45)
}

pub fn cargo_multiplier(cargo_type: CargoType) -> f64 {
    match cargo_type {
        CargoType::Standard => 1.00,
        CargoType::Fragile => 1.06,
        CargoType::Refrigerated => 1.05,
        CargoType::Valuable => 1.10,
        CargoType::Hazardous => 1.14,
        CargoType::Oversize => 1.12,
    }
}

pub fn urgency_multiplier(urgency: Urgency) -> f64 {
    match urgency {
        Urgency::Normal => 1.00,
        Urgency::Priority => 1.08,
        Urgency::Express => 1.16,
    }
}

pub fn market_variation(seed: u64, company_id: &str) -> f64 {
    let mut hash = seed ^ 0x9E37_79B9_7F4A_7C15;
    for byte in company_id.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x1000_0000_01B3);
    }
    let normalized = (hash % 1001) as f64 / 1000.0;
    0.97 + (0.06 * normalized)
}

fn payment_tier_to_db(tier: CompanyPaymentTier) -> &'static str {
    match tier {
        CompanyPaymentTier::Budget => "budget",
        CompanyPaymentTier::Standard => "standard",
        CompanyPaymentTier::Good => "good",
        CompanyPaymentTier::Premium => "premium",
        CompanyPaymentTier::Elite => "elite",
    }
}

fn payment_tier_from_db(value: &str) -> CompanyPaymentTier {
    match value.trim().to_ascii_lowercase().as_str() {
        "budget" => CompanyPaymentTier::Budget,
        "good" => CompanyPaymentTier::Good,
        "premium" => CompanyPaymentTier::Premium,
        "elite" => CompanyPaymentTier::Elite,
        _ => CompanyPaymentTier::Standard,
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let normalized = raw.trim().to_string();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}
