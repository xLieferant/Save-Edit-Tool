use rusqlite::Connection;
use std::sync::atomic::Ordering;
use tauri::command;
use tauri::State;

use crate::features::career::dispatcher::{self, Job};
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
