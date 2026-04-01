use chrono::Utc;
use rusqlite::Connection;

use crate::features::career::job_log::{self, JobLogEntry};
use crate::features::career::telemetry::{JobEvent, TelemetryJob, TelemetrySnapshot};
use crate::state::{ActiveJobEvent, ActiveJobState, CareerRuntime};

fn fallback(value: &str, unknown: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        unknown.to_string()
    } else {
        trimmed.to_string()
    }
}

fn stable_job_fingerprint(job: &TelemetryJob) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        job.source_city.trim(),
        job.destination_city.trim(),
        job.source_company.trim(),
        job.destination_company.trim(),
        job.cargo.trim(),
        job.job_market.trim(),
        job.special_job
    )
}

fn stable_job_fingerprint_from_state(state: &ActiveJobState) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        state.origin_city.trim(),
        state.destination_city.trim(),
        state.source_company.trim(),
        state.destination_company.trim(),
        state.cargo.trim(),
        state.job_market.trim(),
        state.special_job
    )
}

fn fnv1a64(text: &str) -> u64 {
    let mut hash: u64 = 14695981039346656037u64;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211u64);
    }
    hash
}

fn derived_job_id(job: &TelemetryJob) -> String {
    let fingerprint = stable_job_fingerprint(job);
    format!("job-{:016x}", fnv1a64(&fingerprint))
}

pub fn process_snapshot(runtime: &CareerRuntime, snapshot: &TelemetrySnapshot) -> Result<(), String> {
    let Some(job) = snapshot.job.as_ref() else {
        return handle_job_inactive(runtime);
    };

    let incoming_job_id = job.job_id.trim().to_string();
    let has_any_job_signal = !job.source_city.trim().is_empty()
        || !job.destination_city.trim().is_empty()
        || !job.source_company.trim().is_empty()
        || !job.destination_company.trim().is_empty()
        || !job.cargo.trim().is_empty()
        || job.income != 0
        || job.planned_distance_km != 0.0
        || job.delivery_time_min != 0;

    if incoming_job_id.is_empty() && !has_any_job_signal {
        return handle_job_inactive(runtime);
    }

    let now = Utc::now().to_rfc3339();
    let mut guard = runtime
        .active_job
        .lock()
        .map_err(|_| "Career active_job lock poisoned".to_string())?;

    let mut changed = false;
    let next_fingerprint = stable_job_fingerprint(job);
    let mut job_id = if incoming_job_id.is_empty() {
        derived_job_id(job)
    } else {
        incoming_job_id
    };

    if let Some(previous) = guard.as_ref() {
        if previous.job_id != job_id
            && stable_job_fingerprint_from_state(previous) == next_fingerprint
        {
            // Job ID may be unstable depending on the telemetry source; keep the previous id
            // if the identity fingerprint didn't change to avoid duplicate jobs.
            job_id = previous.job_id.clone();
        }
    }

    let unknown_city = "Unbekannte Stadt";
    let unknown_company = "Unbekannte Firma";
    let unknown_cargo = "Unbekannte Fracht";

    let origin_city = fallback(&job.source_city, unknown_city);
    let destination_city = fallback(&job.destination_city, unknown_city);
    let source_company = fallback(&job.source_company, unknown_company);
    let destination_company = fallback(&job.destination_company, unknown_company);
    let cargo = fallback(&job.cargo, unknown_cargo);

    if guard.as_ref().map(|j| j.job_id.as_str()) != Some(job_id.as_str()) {
        if let Some(previous) = guard.as_ref() {
            finalize_job(runtime, previous, &now)?;
        }
        crate::dev_log!(
            "[career] active job detected: {} ({} -> {}, cargo={})",
            job_id,
            origin_city,
            destination_city,
            cargo
        );
        changed = true;
        *guard = Some(ActiveJobState {
            job_id: job_id.clone(),
            started_at_utc: now.clone(),
            last_seen_at_utc: now.clone(),
            origin_city: origin_city.clone(),
            destination_city: destination_city.clone(),
            source_company: source_company.clone(),
            destination_company: destination_company.clone(),
            cargo: cargo.clone(),
            planned_distance_km: job.planned_distance_km,
            income: job.income,
            delivery_time_min: job.delivery_time_min,
            game_time_min: job.game_time_min,
            cargo_damage: job.cargo_damage as f32,
            job_market: job.job_market.clone(),
            special_job: job.special_job,
            last_event: job.event.map(to_state_event),
        });
    }

    if let Some(active) = guard.as_mut() {
        if active.origin_city != origin_city {
            changed = true;
        }
        if active.destination_city != destination_city {
            changed = true;
        }
        if active.source_company != source_company {
            changed = true;
        }
        if active.destination_company != destination_company {
            changed = true;
        }
        if active.cargo != cargo {
            changed = true;
        }
        if (active.planned_distance_km - job.planned_distance_km).abs() > 0.01 {
            changed = true;
        }
        if active.income != job.income {
            changed = true;
        }
        if active.delivery_time_min != job.delivery_time_min {
            changed = true;
        }
        if active.game_time_min != job.game_time_min {
            changed = true;
        }
        if (active.cargo_damage - job.cargo_damage as f32).abs() > 0.001 {
            changed = true;
        }
        if active.job_market != job.job_market {
            changed = true;
        }
        if active.special_job != job.special_job {
            changed = true;
        }

        active.last_seen_at_utc = now.clone();
        active.origin_city = origin_city;
        active.destination_city = destination_city;
        active.source_company = source_company;
        active.destination_company = destination_company;
        active.cargo = cargo;
        active.planned_distance_km = job.planned_distance_km;
        active.income = job.income;
        active.delivery_time_min = job.delivery_time_min;
        active.game_time_min = job.game_time_min;
        active.cargo_damage = job.cargo_damage as f32;
        active.job_market = job.job_market.clone();
        active.special_job = job.special_job;
        let next_event = job.event.map(to_state_event);
        if active.last_event != next_event {
            active.last_event = next_event;
            changed = true;
        }

        upsert_job(runtime, active)?;
    }

    if changed {
        runtime.overview_dirty.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    Ok(())
}

fn handle_job_inactive(runtime: &CareerRuntime) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let mut guard = runtime
        .active_job
        .lock()
        .map_err(|_| "Career active_job lock poisoned".to_string())?;

    if let Some(previous) = guard.take() {
        crate::dev_log!("[career] active job ended: {}", previous.job_id);
        finalize_job(runtime, &previous, &now)?;
        runtime
            .overview_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
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

fn upsert_job(runtime: &CareerRuntime, active: &ActiveJobState) -> Result<(), String> {
    let conn = open_connection(runtime)?;
    job_log::ensure_tables(&conn)?;

    let existing = job_log::get_job(&conn, &active.job_id)?;
    let started_at_utc = existing
        .as_ref()
        .map(|row| row.started_at_utc.clone())
        .unwrap_or_else(|| active.started_at_utc.clone());

    let entry = JobLogEntry {
        job_id: active.job_id.clone(),
        started_at_utc,
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
    };

    job_log::upsert_active_job(&conn, &entry)
}

fn finalize_job(runtime: &CareerRuntime, previous: &ActiveJobState, now: &str) -> Result<(), String> {
    let conn = open_connection(runtime)?;
    job_log::ensure_tables(&conn)?;

    let status = match previous.last_event {
        Some(ActiveJobEvent::Delivered) => "completed",
        Some(ActiveJobEvent::Cancelled) => "aborted",
        None => "unknown",
    };

    job_log::mark_job_finished(
        &conn,
        &previous.job_id,
        now,
        status,
        previous.cargo_damage as f64,
    )
}

fn to_state_event(event: JobEvent) -> ActiveJobEvent {
    match event {
        JobEvent::Delivered => ActiveJobEvent::Delivered,
        JobEvent::Cancelled => ActiveJobEvent::Cancelled,
    }
}
