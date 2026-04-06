use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashSet;

use crate::features::career::dispatcher::{self, Job};
use crate::features::career::job_log::{self, JobLogEntry, JobStats};
use crate::features::career::logbook::{self, ActiveTripView, TripSummary};
use crate::features::economy::compensation_models::{
    CompanyCompensationCondition, CountryPaymentLevel,
};
use crate::features::economy::compensation_service;
use crate::features::{bank, contracts, economy, employees, events, fleet, reputation};
use crate::state::{CareerRuntime, LiveTelemetryState};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerDashboardMetrics {
    pub live_income: i64,
    pub fuel_cost: i64,
    pub repair_cost: i64,
    pub toll_cost: i64,
    pub drivers_online: i64,
    pub drivers_resting: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerStatistics {
    pub total_trips: i64,
    pub total_kilometers: f64,
    pub total_income: i64,
    pub average_speed: f64,
    pub speeding_events: i64,
    pub company_value: i64,
    pub completed_trips: i64,
    pub completed_jobs: i64,
    pub failed_jobs: i64,
    pub cancelled_jobs: i64,
    pub abandoned_jobs: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CareerOverview {
    pub economy: economy::EconomyState,
    pub bank: bank::BankState,
    pub reputation: reputation::ReputationState,
    pub employees: Vec<employees::EmployeeSummary>,
    pub employee_overview: employees::EmployeeOverview,
    pub fleet: Vec<fleet::FleetAssetSummary>,
    pub fleet_overview: fleet::FleetOverview,
    pub contracts: Vec<contracts::ContractSummary>,
    pub dispatcher_events: Vec<events::CareerEvent>,
    pub freight_offers: Vec<economy::FreightOffer>,
    pub company_conditions: Vec<CompanyCompensationCondition>,
    pub country_payment_levels: Vec<CountryPaymentLevel>,
    pub jobs: Vec<Job>,
    pub current_job: Option<Job>,
    pub active_trip: Option<ActiveTripView>,
    pub recent_trips: Vec<TripSummary>,
    pub active_job: Option<JobLogEntry>,
    pub recent_jobs: Vec<JobLogEntry>,
    pub job_stats: JobStats,
    pub last_telemetry: Option<LiveTelemetryState>,
    pub dashboard: CareerDashboardMetrics,
    pub statistics: CareerStatistics,
}

pub fn load_overview(runtime: &CareerRuntime) -> Result<CareerOverview, String> {
    let db_path = runtime
        .db_path
        .lock()
        .map_err(|_| "Career db_path lock poisoned".to_string())?
        .clone()
        .ok_or_else(|| "Career database path not initialized".to_string())?;

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    let economy_state = economy::load_state(&conn)?;
    let bank_state = bank::load_state(&conn)?;
    let reputation_state = reputation::load_state(&conn)?;
    let employees_list = employees::load_staff(&conn, 8)?;
    let employee_overview = employees::load_overview(&conn)?;
    let fleet_assets = fleet::load_assets(&conn, 8)?;
    let fleet_overview = fleet::load_overview(&conn)?;
    let contracts_list = contracts::load_active_contracts(&conn, 6)?;
    let dispatcher_events = events::list_recent_events(&conn, 6)?;
    let freight_offers = economy::list_freight_offers(&conn, 6)?;
    let company_conditions = compensation_service::list_company_compensation_conditions(&conn, 12)?;
    let country_payment_levels = compensation_service::list_country_payment_levels(&conn, 24)?;
    let recent_trips = logbook::list_trips_from_connection(&conn, 8)?;
    let active_trip = logbook::current_active_trip(runtime)?;
    job_log::ensure_tables(&conn)?;
    let mut recent_jobs = job_log::list_recent_jobs(&conn, 8)?;
    job_log::enrich_job_entries(&conn, &mut recent_jobs)?;
    let job_stats = job_log::load_job_stats(&conn)?;
    let mut jobs = dispatcher::list_jobs(&conn, 8)?;
    let mut current_job = dispatcher::current_job(&conn)?;
    let last_telemetry = runtime
        .last_telemetry
        .lock()
        .map_err(|_| "Career last_telemetry lock poisoned".to_string())?
        .clone();

    apply_live_job_progress(&mut jobs, current_job.as_mut(), active_trip.as_ref());

    let mut active_job = runtime
        .active_job
        .lock()
        .map_err(|_| "Career active_job lock poisoned".to_string())?
        .as_ref()
        .map(|active| JobLogEntry {
            job_id: active.job_id.clone(),
            started_at_utc: active.started_at_utc.clone(),
            ended_at_utc: None,
            origin_city: active.origin_city.clone(),
            destination_city: active.destination_city.clone(),
            source_company: active.source_company.clone(),
            destination_company: active.destination_company.clone(),
            cargo: active.cargo.clone(),
            planned_distance_km: active.planned_distance_km,
            income: active.income,
            delivery_time_min: active.delivery_time_min,
            game_time_min: Some(active.game_time_min),
            remaining_time_min: Some(active.delivery_time_min as i64 - active.game_time_min as i64),
            last_seen_at_utc: active.last_seen_at_utc.clone(),
            status: "active".to_string(),
            cargo_damage: active.cargo_damage as f64,
            job_market: active.job_market.clone(),
            special_job: active.special_job,
            ingame_income: None,
            vtc_expected_income: None,
            result_status: None,
            planned_distance_source: None,
        });
    if let Some(active_job) = active_job.as_mut() {
        job_log::enrich_job_entry(&conn, active_job)?;
    }

    let statistics = load_statistics(&conn, &bank_state)?;
    let dashboard = build_dashboard(
        &recent_trips,
        &employee_overview,
        &fleet_overview,
        &economy_state,
    );

    Ok(CareerOverview {
        economy: economy_state,
        bank: bank_state,
        reputation: reputation_state,
        employees: employees_list,
        employee_overview,
        fleet: fleet_assets,
        fleet_overview,
        contracts: contracts_list,
        dispatcher_events,
        freight_offers,
        company_conditions,
        country_payment_levels,
        jobs,
        current_job,
        active_trip,
        recent_trips,
        active_job,
        recent_jobs,
        job_stats,
        last_telemetry,
        dashboard,
        statistics,
    })
}

fn apply_live_job_progress(
    jobs: &mut [Job],
    current_job: Option<&mut Job>,
    active_trip: Option<&ActiveTripView>,
) {
    let Some(active_trip) = active_trip else {
        return;
    };

    let apply_progress = |job: &mut Job| {
        if job.id != active_trip.job_id || job.completed {
            return;
        }
        let live_progress = (job.progress_km + active_trip.distance_km).min(job.distance_km);
        job.progress_km = live_progress;
        job.remaining_km = (job.distance_km - live_progress).max(0.0);
    };

    for job in jobs.iter_mut() {
        apply_progress(job);
    }

    if let Some(job) = current_job {
        apply_progress(job);
    }
}

fn build_dashboard(
    recent_trips: &[TripSummary],
    employees: &employees::EmployeeOverview,
    fleet_overview: &fleet::FleetOverview,
    economy_state: &economy::EconomyState,
) -> CareerDashboardMetrics {
    let recent_income = recent_trips
        .iter()
        .take(4)
        .filter_map(|trip| trip.income)
        .sum::<i64>();
    let recent_fuel = recent_trips
        .iter()
        .take(4)
        .map(|trip| (trip.fuel_used_liters * economy_state.diesel_price_per_liter).round() as i64)
        .sum::<i64>();
    let recent_tolls = recent_trips
        .iter()
        .take(4)
        .map(|trip| (trip.distance_km * economy_state.toll_per_km).round() as i64)
        .sum::<i64>();
    let repair_cost = if fleet_overview.player_condition < 82.0 {
        ((82.0 - fleet_overview.player_condition) * 22.0).round() as i64
    } else {
        0
    };

    CareerDashboardMetrics {
        live_income: recent_income.max(0),
        fuel_cost: recent_fuel.max(0),
        repair_cost,
        toll_cost: recent_tolls.max(0),
        drivers_online: employees.on_duty.max(0),
        drivers_resting: employees.resting.max(0),
    }
}

fn load_statistics(
    conn: &Connection,
    bank_state: &bank::BankState,
) -> Result<CareerStatistics, String> {
    let mut jobs = job_log::list_recent_jobs(conn, 5000)?;
    job_log::enrich_job_entries(conn, &mut jobs)?;

    let mut seen_job_ids = HashSet::new();
    let mut total_trips = 0_i64;
    let mut total_kilometers = 0.0_f64;
    let mut total_income = 0_i64;
    let mut completed_jobs = 0_i64;
    let mut failed_jobs = 0_i64;
    let mut cancelled_jobs = 0_i64;
    let mut abandoned_jobs = 0_i64;

    for job in &jobs {
        total_trips += 1;
        total_kilometers += job.planned_distance_km.max(0.0);
        if !job.job_id.trim().is_empty() {
            seen_job_ids.insert(job.job_id.clone());
        }

        match job.status.as_str() {
            "completed" => {
                completed_jobs += 1;
                total_income += job
                    .vtc_expected_income
                    .or(job.ingame_income)
                    .unwrap_or(job.income);
            }
            "failed" => failed_jobs += 1,
            "cancelled" => cancelled_jobs += 1,
            "abandoned" => abandoned_jobs += 1,
            _ => {}
        }
    }

    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                job_id,
                COALESCE(distance_km, 0),
                income,
                COALESCE(status, 'completed')
            FROM trips
            "#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (job_id, distance_km, income, status) = row.map_err(|e| e.to_string())?;
        let already_counted = job_id
            .as_deref()
            .map(|value| seen_job_ids.contains(value))
            .unwrap_or(false);
        if already_counted {
            continue;
        }

        total_trips += 1;
        total_kilometers += distance_km.max(0.0);
        match status.as_str() {
            "completed" => {
                completed_jobs += 1;
                total_income += income.unwrap_or(0);
            }
            "failed" => failed_jobs += 1,
            "cancelled" => cancelled_jobs += 1,
            "paused" | "aborted" => abandoned_jobs += 1,
            _ => {}
        }
    }

    conn.query_row(
        r#"
        SELECT
            COALESCE(AVG(NULLIF(avg_speed_kph, 0)), 0),
            COALESCE(SUM(speeding_events), 0)
        FROM trips
        "#,
        [],
        |row| {
            Ok(CareerStatistics {
                total_trips,
                total_kilometers,
                total_income,
                average_speed: row.get(0)?,
                speeding_events: row.get(1)?,
                company_value: bank_state.cash_balance + total_income,
                completed_trips: completed_jobs,
                completed_jobs,
                failed_jobs,
                cancelled_jobs,
                abandoned_jobs,
            })
        },
    )
    .map_err(|e| e.to_string())
}
