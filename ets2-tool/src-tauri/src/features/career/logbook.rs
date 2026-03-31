use std::path::Path;
use std::sync::atomic::Ordering;

use chrono::Utc;
use rusqlite::{Connection, params};
use serde::Serialize;

use crate::features::career::dispatcher;
use crate::features::{bank, contracts, economy, employees, events, fleet, reputation};
use crate::state::{ActiveTripState, CareerRuntime, LiveTelemetryState};

#[derive(Debug, Clone, Copy)]
pub struct TelemetrySample {
    pub timestamp: u64,
    pub speed_kph: f32,
    pub rpm: f32,
    pub gear: i32,
    pub fuel_liters: f32,
    pub fuel_capacity_liters: f32,
    pub engine_enabled: bool,
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TripSummary {
    pub id: i64,
    pub job_id: Option<String>,
    pub started_at_utc: String,
    pub ended_at_utc: Option<String>,
    pub origin: Option<String>,
    pub destination: Option<String>,
    pub cargo: Option<String>,
    pub distance_km: f64,
    pub income: Option<i64>,
    pub damage: f64,
    pub duration_seconds: i64,
    pub avg_speed_kph: f64,
    pub max_speed_kph: f64,
    pub speeding_events: i64,
    pub fuel_used_liters: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTripView {
    pub trip_id: i64,
    pub job_id: String,
    pub contract_id: Option<String>,
    pub origin: String,
    pub destination: String,
    pub cargo: String,
    pub distance_km: f64,
    pub duration_seconds: i64,
    pub avg_speed_kph: f64,
    pub max_speed_kph: f32,
    pub speeding_events: i64,
    pub fuel_used_liters: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalizeReason {
    EngineOff,
    JobCompleted,
}

pub fn process_snapshot(runtime: &CareerRuntime, sample: TelemetrySample) -> Result<(), String> {
    update_runtime_telemetry(runtime, sample)?;

    if sample.timestamp == 0 {
        return Ok(());
    }

    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let mut pending_finalize: Option<(ActiveTripState, FinalizeReason)> = None;

    {
        let mut active_guard = runtime
            .active_trip
            .lock()
            .map_err(|_| "Career active_trip lock poisoned".to_string())?;

        if let Some(active) = active_guard.as_mut() {
            update_active_trip(active, sample, now_ms);
            runtime.overview_dirty.store(true, Ordering::Relaxed);

            if let Some(target_distance_km) = active.job_target_distance_km {
                let total_progress = active.job_progress_base_km + active.distance_km;
                if total_progress >= target_distance_km {
                    pending_finalize = Some((active.clone(), FinalizeReason::JobCompleted));
                } else if !sample.engine_enabled {
                    pending_finalize = Some((active.clone(), FinalizeReason::EngineOff));
                }
            } else if !sample.engine_enabled {
                pending_finalize = Some((active.clone(), FinalizeReason::EngineOff));
            }
        }
    }

    if let Some((active, reason)) = pending_finalize {
        finalize_trip(runtime, &active, sample, reason)?;
        let mut active_guard = runtime
            .active_trip
            .lock()
            .map_err(|_| "Career active_trip lock poisoned".to_string())?;
        if active_guard.as_ref().map(|entry| entry.trip_id) == Some(active.trip_id) {
            *active_guard = None;
        }
        if reason == FinalizeReason::JobCompleted && sample.engine_enabled {
            runtime.trip_start_blocked.store(true, Ordering::Relaxed);
        }
        runtime.overview_dirty.store(true, Ordering::Relaxed);
        return Ok(());
    }

    if runtime.trip_start_blocked.load(Ordering::Relaxed) {
        if !sample.engine_enabled || sample.speed_kph < 0.5 {
            runtime.trip_start_blocked.store(false, Ordering::Relaxed);
        }
        return Ok(());
    }

    if sample.engine_enabled && !sample.paused && sample.speed_kph > 0.5 {
        let should_start = {
            let active_guard = runtime
                .active_trip
                .lock()
                .map_err(|_| "Career active_trip lock poisoned".to_string())?;
            active_guard.is_none()
        };

        if should_start {
            start_trip(runtime, sample, now)?;
            runtime.overview_dirty.store(true, Ordering::Relaxed);
        }
    }

    Ok(())
}

pub fn current_active_trip(runtime: &CareerRuntime) -> Result<Option<ActiveTripView>, String> {
    let active_guard = runtime
        .active_trip
        .lock()
        .map_err(|_| "Career active_trip lock poisoned".to_string())?;

    Ok(active_guard.as_ref().map(|trip| ActiveTripView {
        trip_id: trip.trip_id,
        job_id: trip.job_id.clone(),
        contract_id: trip.contract_id.clone(),
        origin: trip.origin.clone(),
        destination: trip.destination.clone(),
        cargo: trip.cargo.clone(),
        distance_km: trip.distance_km,
        duration_seconds: trip.duration_seconds,
        avg_speed_kph: if trip.speed_samples > 0 {
            trip.speed_sum_kph / trip.speed_samples as f64
        } else {
            0.0
        },
        max_speed_kph: trip.max_speed_kph,
        speeding_events: trip.speeding_events,
        fuel_used_liters: trip.fuel_used_liters,
    }))
}

pub fn list_trips(db_path: &Path, limit: usize) -> Result<Vec<TripSummary>, String> {
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    list_trips_from_connection(&conn, limit)
}

pub fn list_trips_from_connection(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<TripSummary>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                job_id,
                started_at_utc,
                ended_at_utc,
                origin,
                destination,
                cargo,
                COALESCE(distance_km, 0),
                income,
                COALESCE(damage, 0),
                COALESCE(duration_seconds, 0),
                COALESCE(avg_speed_kph, 0),
                COALESCE(max_speed_kph, 0),
                COALESCE(speeding_events, 0),
                COALESCE(fuel_used_liters, 0),
                COALESCE(status, 'completed')
            FROM trips
            ORDER BY id DESC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(TripSummary {
                id: row.get(0)?,
                job_id: row.get(1)?,
                started_at_utc: row.get(2)?,
                ended_at_utc: row.get(3)?,
                origin: row.get(4)?,
                destination: row.get(5)?,
                cargo: row.get(6)?,
                distance_km: row.get(7)?,
                income: row.get(8)?,
                damage: row.get(9)?,
                duration_seconds: row.get(10)?,
                avg_speed_kph: row.get(11)?,
                max_speed_kph: row.get(12)?,
                speeding_events: row.get(13)?,
                fuel_used_liters: row.get(14)?,
                status: row.get(15)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn start_trip(
    runtime: &CareerRuntime,
    sample: TelemetrySample,
    now: chrono::DateTime<Utc>,
) -> Result<(), String> {
    let conn = open_connection(runtime)?;
    let active_job = dispatcher::current_assignment(&conn)?;
    let assignment = if let Some(job) = active_job.clone() {
        contracts::DispatchAssignment {
            job_id: job.id,
            contract_id: None,
            company_name: "Dispatcher".to_string(),
            origin: job.source,
            destination: job.destination,
            cargo: job.cargo,
            bonus_payout: 0,
        }
    } else {
        contracts::select_dispatch_assignment(&conn)?
    };
    employees::mark_driver_status(&conn, "on_duty")?;

    conn.execute(
        r#"
        INSERT INTO trips (
            job_id,
            contract_id,
            started_at_utc,
            origin,
            destination,
            cargo,
            distance_km,
            income,
            damage,
            duration_seconds,
            avg_speed_kph,
            max_speed_kph,
            speeding_events,
            fuel_used_liters,
            status,
            raw_telemetry_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, 0, 0, 0, 0, 0, 0, 'active', ?7)
        "#,
        params![
            assignment.job_id,
            assignment.contract_id,
            now.to_rfc3339(),
            assignment.origin,
            assignment.destination,
            assignment.cargo,
            telemetry_json(sample)
        ],
    )
    .map_err(|e| e.to_string())?;

    let trip_id = conn.last_insert_rowid();
    let speeding = if sample.speed_kph > 90.0 { 1 } else { 0 };
    let state = ActiveTripState {
        trip_id,
        job_id: assignment.job_id,
        contract_id: assignment.contract_id,
        job_progress_base_km: active_job.as_ref().map(|job| job.progress_km).unwrap_or(0.0),
        job_target_distance_km: active_job.as_ref().map(|job| job.distance_km),
        job_price_per_km: active_job.as_ref().map(|job| job.price_per_km),
        started_at_utc_ms: now.timestamp_millis(),
        last_update_utc_ms: now.timestamp_millis(),
        origin: assignment.origin,
        destination: assignment.destination,
        cargo: assignment.cargo,
        bonus_payout: assignment.bonus_payout,
        distance_km: 0.0,
        duration_seconds: 0,
        max_speed_kph: sample.speed_kph,
        speed_sum_kph: sample.speed_kph as f64,
        speed_samples: 1,
        speeding_events: speeding,
        was_speeding: sample.speed_kph > 90.0,
        fuel_used_liters: 0.0,
        last_fuel_liters: sample.fuel_liters,
        last_speed_kph: sample.speed_kph,
    };

    let mut active_guard = runtime
        .active_trip
        .lock()
        .map_err(|_| "Career active_trip lock poisoned".to_string())?;
    *active_guard = Some(state);
    Ok(())
}

fn finalize_trip(
    runtime: &CareerRuntime,
    active: &ActiveTripState,
    sample: TelemetrySample,
    reason: FinalizeReason,
) -> Result<(), String> {
    let conn = open_connection(runtime)?;
    let ended_at = Utc::now();
    let duration_seconds = ((ended_at.timestamp_millis() - active.started_at_utc_ms) / 1000).max(0);
    let average_speed = if active.speed_samples > 0 {
        active.speed_sum_kph / active.speed_samples as f64
    } else {
        0.0
    };

    let job_target_distance = active.job_target_distance_km.unwrap_or(0.0);
    let job_total_progress = active.job_progress_base_km + active.distance_km;
    let is_dispatch_job = active.job_target_distance_km.is_some() && active.job_price_per_km.is_some();
    let dispatcher_completed =
        is_dispatch_job && (reason == FinalizeReason::JobCompleted || job_total_progress >= job_target_distance);
    let mut trip_status = "completed";
    let mut income: Option<i64> = None;

    if is_dispatch_job {
        dispatcher::store_progress(&conn, &active.job_id, job_total_progress.min(job_target_distance))?;
    }

    if !is_dispatch_job || dispatcher_completed {
        let distance_for_result = if dispatcher_completed {
            job_target_distance.max(active.distance_km)
        } else {
            active.distance_km
        };
        let wear = fleet::apply_trip_wear(&conn, distance_for_result, active.speeding_events)?;
        let costs = economy::estimate_trip_costs(
            &conn,
            active.fuel_used_liters,
            distance_for_result,
            wear.repair_reserve,
        )?;
        let gross_income = if dispatcher_completed {
            (job_target_distance * active.job_price_per_km.unwrap_or_default()).round() as i64
        } else {
            economy::estimate_trip_revenue(&conn, distance_for_result, active.bonus_payout)?
        };
        let net_income = gross_income - costs.total_cost;

        income = Some(net_income);
        let bank_state = bank::apply_trip_result(&conn, net_income)?;
        let reputation_state =
            reputation::apply_trip_outcome(&conn, distance_for_result, active.speeding_events)?;

        if dispatcher_completed {
            let _ = dispatcher::complete_job(&conn, &active.job_id)?;
        } else {
            contracts::record_completion(&conn, active.contract_id.as_deref())?;
        }
        employees::mark_driver_status(&conn, "resting")?;

        events::record_event(
            &conn,
            "dispatcher",
            "Trip completed",
            &format!(
                "{} -> {} delivered. Net result: EUR {}.",
                active.origin, active.destination, net_income
            ),
            "low",
        )?;

        if active.speeding_events > 0 {
            events::record_event(
                &conn,
                "compliance",
                "Speeding violation detected",
                &format!(
                    "{} speeding event(s) recorded on route {} -> {}.",
                    active.speeding_events, active.origin, active.destination
                ),
                "high",
            )?;
        }

        if wear.player_condition < 82.0 {
            events::record_event(
                &conn,
                "fleet",
                "Maintenance required",
                &format!(
                    "Player truck condition dropped to {:.0}% and needs workshop attention.",
                    wear.player_condition
                ),
                "medium",
            )?;
        }

        if bank_state.debt_balance > 0 {
            events::record_event(
                &conn,
                "bank",
                "Loan installment processed",
                &format!(
                    "Debt is now EUR {} after the latest repayment cycle.",
                    bank_state.debt_balance
                ),
                "medium",
            )?;
        }

        if reputation_state.level > 1 && reputation_state.completed_jobs % 3 == 0 {
            events::record_event(
                &conn,
                "reputation",
                "Reputation increased",
                &format!(
                    "Level {} reached with {} completed jobs.",
                    reputation_state.level, reputation_state.completed_jobs
                ),
                "low",
            )?;
        }
    } else {
        trip_status = "paused";
        employees::mark_driver_status(&conn, "resting")?;
        events::record_event(
            &conn,
            "dispatcher",
            "Job progress saved",
            &format!(
                "{} -> {} is at {:.1}/{:.1} km.",
                active.origin,
                active.destination,
                job_total_progress.min(job_target_distance),
                job_target_distance
            ),
            "low",
        )?;
    }

    conn.execute(
        r#"
        UPDATE trips
        SET
            ended_at_utc = ?1,
            origin = ?2,
            destination = ?3,
            cargo = ?4,
            distance_km = ?5,
            income = ?6,
            damage = 0,
            duration_seconds = ?7,
            avg_speed_kph = ?8,
            max_speed_kph = ?9,
            speeding_events = ?10,
            fuel_used_liters = ?11,
            status = ?12,
            raw_telemetry_json = ?13
        WHERE id = ?14
        "#,
        params![
            ended_at.to_rfc3339(),
            active.origin,
            active.destination,
            active.cargo,
            active.distance_km,
            income,
            duration_seconds,
            average_speed,
            active.max_speed_kph,
            active.speeding_events,
            active.fuel_used_liters,
            trip_status,
            telemetry_json(sample),
            active.trip_id
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn update_active_trip(active: &mut ActiveTripState, sample: TelemetrySample, now_ms: i64) {
    if !sample.paused {
        let delta_ms = (now_ms - active.last_update_utc_ms).clamp(0, 3000);
        let delta_hours = delta_ms as f64 / 3_600_000.0;
        let average_speed = ((active.last_speed_kph + sample.speed_kph) as f64 / 2.0).max(0.0);

        active.distance_km += average_speed * delta_hours;
        active.duration_seconds = ((now_ms - active.started_at_utc_ms) / 1000).max(0);
        active.max_speed_kph = active.max_speed_kph.max(sample.speed_kph);
        active.speed_sum_kph += sample.speed_kph as f64;
        active.speed_samples = active.speed_samples.saturating_add(1);

        if sample.speed_kph > 90.0 {
            if !active.was_speeding {
                active.speeding_events += 1;
            }
            active.was_speeding = true;
        } else if sample.speed_kph < 88.0 {
            active.was_speeding = false;
        }

        if sample.fuel_liters < active.last_fuel_liters {
            active.fuel_used_liters += (active.last_fuel_liters - sample.fuel_liters) as f64;
        }
    }

    active.last_fuel_liters = sample.fuel_liters;
    active.last_speed_kph = sample.speed_kph;
    active.last_update_utc_ms = now_ms;
}

fn update_runtime_telemetry(runtime: &CareerRuntime, sample: TelemetrySample) -> Result<(), String> {
    let mut telemetry_guard = runtime
        .last_telemetry
        .lock()
        .map_err(|_| "Career last_telemetry lock poisoned".to_string())?;

    let next = LiveTelemetryState {
        speed_kph: sample.speed_kph,
        rpm: sample.rpm,
        gear: format_gear(sample.gear),
        fuel_liters: sample.fuel_liters,
        fuel_capacity_liters: sample.fuel_capacity_liters,
        engine_on: sample.engine_enabled,
        timestamp: sample.timestamp,
        paused: sample.paused,
    };

    if telemetry_guard.as_ref() != Some(&next) {
        *telemetry_guard = Some(next);
        runtime.overview_dirty.store(true, Ordering::Relaxed);
    }

    Ok(())
}

fn open_connection(runtime: &CareerRuntime) -> Result<Connection, String> {
    let db_path = runtime
        .db_path
        .lock()
        .map_err(|_| "Career db_path lock poisoned".to_string())?
        .clone()
        .ok_or_else(|| "Career database path not initialized".to_string())?;

    Connection::open(db_path).map_err(|e| e.to_string())
}

fn telemetry_json(sample: TelemetrySample) -> String {
    serde_json::json!({
        "timestamp": sample.timestamp,
        "speedKph": sample.speed_kph,
        "rpm": sample.rpm,
        "gear": format_gear(sample.gear),
        "fuelLiters": sample.fuel_liters,
        "fuelCapacityLiters": sample.fuel_capacity_liters,
        "engineOn": sample.engine_enabled,
        "paused": sample.paused,
    })
    .to_string()
}

fn format_gear(gear: i32) -> String {
    match gear {
        value if value < 0 => format!("R{}", value.abs()),
        0 => "N".to_string(),
        value => value.to_string(),
    }
}
