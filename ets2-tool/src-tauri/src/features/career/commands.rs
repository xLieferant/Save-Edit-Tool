use rusqlite::Connection;
use std::sync::atomic::Ordering;
use tauri::command;
use tauri::State;

use crate::features::career::dispatcher::{self, Job};
use crate::features::career::job_log::{self, JobLogEntry, JobStats};
use crate::features::career::logbook::TripSummary;
use crate::features::career::overview::CareerOverview;
use crate::features::career::plugin_installer::{self, ScsGame};
use crate::features::hub::events::CareerStatus;
use crate::state::{AppProfileState, CareerRuntime, CareerState};

#[command]
pub fn career_get_status(career: State<'_, CareerState>) -> Result<CareerStatus, String> {
    let runtime = career.runtime.as_ref();
    crate::dev_log!("[career] command: career_get_status");
    Ok(CareerStatus {
        ets2_running: runtime.ets2_running.load(Ordering::Relaxed),
        ats_running: runtime.ats_running.load(Ordering::Relaxed),
        telemetry_running: runtime.telemetry_running.load(Ordering::Relaxed),
        plugin_installed: runtime.plugin_installed.load(Ordering::Relaxed),
        bridge_connected: runtime.bridge_connected.load(Ordering::Relaxed),
        active_game: runtime
            .active_game
            .lock()
            .map_err(|_| "Career active_game lock poisoned".to_string())?
            .clone(),
    })
}

#[command]
pub fn get_plugin_status(profile: State<'_, AppProfileState>) -> Result<bool, String> {
    let selected_game = profile
        .selected_game
        .lock()
        .map_err(|_| "AppProfileState selected_game lock poisoned".to_string())?
        .clone();

    let game = ScsGame::try_from(selected_game.as_str())?;
    Ok(plugin_installer::plugin_file_installed(game).unwrap_or(false))
}

#[command]
pub fn career_get_overview(career: State<'_, CareerState>) -> Result<CareerOverview, String> {
    crate::dev_log!("[career] command: career_get_overview");
    crate::features::career::overview::load_overview(career.runtime.as_ref())
}

#[command]
pub fn career_list_trips(career: State<'_, CareerState>) -> Result<Vec<TripSummary>, String> {
    let runtime = career.runtime.as_ref();
    crate::dev_log!("[career] command: career_list_trips");
    let db_path = runtime
        .db_path
        .lock()
        .map_err(|_| "Career db_path lock poisoned".to_string())?
        .clone()
        .ok_or_else(|| "Career database path not initialized".to_string())?;

    crate::features::career::logbook::list_trips(&db_path, 200)
}

#[command]
pub fn career_get_active_job(career: State<'_, CareerState>) -> Result<Option<JobLogEntry>, String> {
    let runtime = career.runtime.as_ref();
    let guard = runtime
        .active_job
        .lock()
        .map_err(|_| "Career active_job lock poisoned".to_string())?;

    Ok(guard.as_ref().map(|active| JobLogEntry {
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
    }))
}

#[command]
pub fn career_get_job_log(career: State<'_, CareerState>) -> Result<Vec<JobLogEntry>, String> {
    let runtime = career.runtime.as_ref();
    crate::dev_log!("[career] command: career_get_job_log");
    let db_path = runtime
        .db_path
        .lock()
        .map_err(|_| "Career db_path lock poisoned".to_string())?
        .clone()
        .ok_or_else(|| "Career database path not initialized".to_string())?;

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    job_log::ensure_tables(&conn)?;
    job_log::list_recent_jobs(&conn, 200)
}

#[command]
pub fn career_get_job_stats(career: State<'_, CareerState>) -> Result<JobStats, String> {
    let runtime = career.runtime.as_ref();
    crate::dev_log!("[career] command: career_get_job_stats");
    let db_path = runtime
        .db_path
        .lock()
        .map_err(|_| "Career db_path lock poisoned".to_string())?
        .clone()
        .ok_or_else(|| "Career database path not initialized".to_string())?;

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    job_log::ensure_tables(&conn)?;
    job_log::load_job_stats(&conn)
}

#[command]
pub fn career_generate_jobs(career: State<'_, CareerState>) -> Result<Vec<Job>, String> {
    crate::dev_log!("[career] command: career_generate_jobs");
    let runtime = career.runtime.as_ref();
    let conn = open_connection(runtime)?;
    let jobs = dispatcher::generate_jobs(&conn)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(jobs)
}

#[command]
pub fn career_accept_job(job_id: String, career: State<'_, CareerState>) -> Result<Job, String> {
    crate::dev_log!("[career] command: career_accept_job -> {}", job_id);
    let runtime = career.runtime.as_ref();
    let conn = open_connection(runtime)?;
    let job = dispatcher::accept_job(&conn, &job_id)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(job)
}

#[command]
pub fn career_complete_job(
    job_id: String,
    career: State<'_, CareerState>,
) -> Result<Job, String> {
    crate::dev_log!("[career] command: career_complete_job -> {}", job_id);
    let runtime = career.runtime.as_ref();
    let conn = open_connection(runtime)?;
    let job = dispatcher::complete_job(&conn, &job_id)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(job)
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
