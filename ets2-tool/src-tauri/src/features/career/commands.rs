use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::command;
use tauri::{AppHandle, Emitter, State};

use crate::events::{
    EVT_DISPATCHER_ASSIGN_PREPARE_ERROR, EVT_DISPATCHER_ASSIGN_PREPARE_PROGRESS,
    EVT_DISPATCHER_JOB_UPDATED,
};
use crate::features::career::dispatcher::{
    self, DispatcherCompanyContact, DispatcherCreateOfferInput, DispatcherGenerationConfigInput,
    DispatcherGenerationStatus, DispatcherHistoryResponse, DispatcherJobDetails,
    DispatcherJobFilter, DispatcherJobsBySaveContextResponse, DispatcherMarketJob, DispatcherOffer,
    DispatcherOverview, DispatcherRespondToCounterInput, Job,
};
use crate::features::career::job_log::{self, JobLogEntry, JobStats};
use crate::features::career::logbook::TripSummary;
use crate::features::career::overview::CareerOverview;
use crate::features::career::plugin_installer::{self, ScsGame};
use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::features::ets2save::link_service;
use crate::features::ets2save::models::{EtsJobLink, EtsJobLinkStatus, EtsJobWriteResult};
use crate::features::hub::events::CareerStatus;
use crate::shared::current_profile::snapshot_save_context;
use crate::state::{AppProfileState, CareerRuntime, CareerState, EtsDbState};

const EVT_DISPATCHER_ASSIGN_PROGRESS: &str = "vtc://dispatcher/assign_progress";
const EVT_DISPATCHER_PREPARE_PROGRESS: &str = "vtc://dispatcher/prepare_progress";
const EVT_DISPATCHER_WRITE_PROGRESS: &str = "vtc://dispatcher/write_progress";
const EVT_DISPATCHER_JOB_UPDATED_NEW: &str = "vtc://dispatcher/job_updated";
const EVT_DISPATCHER_JOB_ERROR: &str = "vtc://dispatcher/job_error";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherAssignPrepareWriteDto {
    pub job: DispatcherJobDetails,
    pub link: Option<EtsJobLink>,
    pub auto_write: bool,
    pub write_attempted: bool,
    pub write_applied: bool,
    pub dispatcher_status: String,
    pub ets2_job_link_status: Option<String>,
    pub sha_before: Option<String>,
    pub sha_after: Option<String>,
    pub sha_changed: Option<bool>,
    pub save_reference: Option<String>,
    pub quicksave_reference: Option<String>,
    pub save_session_id: Option<String>,
    pub assign_result: String,
    pub prepare_result: String,
    pub write_result: String,
    pub write_output: Option<DispatcherWriteOutputDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatcherWriteOutputDto {
    pub job_id: String,
    pub save_path: String,
    pub backup_path: String,
    pub company_pointer: String,
    pub offer_pointer: Option<String>,
    pub job_offer_data_pointer: Option<String>,
    pub origin: String,
    pub destination: String,
    pub target_company: Option<String>,
    pub cargo_token: Option<String>,
    pub shortest_distance_km: Option<i64>,
    pub expiration_time: Option<i64>,
    pub reward: i64,
    pub write_mode: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub job_info_updated: bool,
    pub final_link_status: String,
}

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
pub fn career_get_active_job(
    career: State<'_, CareerState>,
) -> Result<Option<JobLogEntry>, String> {
    let runtime = career.runtime.as_ref();
    let guard = runtime
        .active_job
        .lock()
        .map_err(|_| "Career active_job lock poisoned".to_string())?;
    let mut active_entry = guard.as_ref().map(|active| JobLogEntry {
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
    drop(guard);

    if let Some(entry) = active_entry.as_mut() {
        let conn = open_connection(runtime)?;
        job_log::ensure_tables(&conn)?;
        job_log::enrich_job_entry(&conn, entry)?;
    }

    Ok(active_entry)
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
    let mut jobs = job_log::list_recent_jobs(&conn, 200)?;
    job_log::enrich_job_entries(&conn, &mut jobs)?;
    Ok(jobs)
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
pub fn career_complete_job(job_id: String, career: State<'_, CareerState>) -> Result<Job, String> {
    crate::dev_log!("[career] command: career_complete_job -> {}", job_id);
    let runtime = career.runtime.as_ref();
    let conn = open_connection(runtime)?;
    let job = dispatcher::complete_job(&conn, &job_id)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(job)
}

#[command]
pub fn dispatcher_get_market_jobs(
    filter: Option<DispatcherJobFilter>,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<Vec<DispatcherMarketJob>, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_market_jobs(&conn, filter, &save_context)
}

#[command]
pub fn dispatcher_get_open_jobs(
    filter: Option<DispatcherJobFilter>,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<Vec<DispatcherMarketJob>, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_open_jobs(&conn, filter, &save_context)
}

#[command]
pub fn dispatcher_get_job_details(
    job_id: String,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherJobDetails, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_job_details(&conn, &job_id, &save_context)
}

#[command]
pub fn dispatcher_get_job_by_id(
    job_id: String,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherJobDetails, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_job_by_id(&conn, &job_id, &save_context)
}

#[command]
pub fn dispatcher_accept_job(
    job_id: String,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherJobDetails, String> {
    let runtime = career.runtime.as_ref();
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_accept_job(&conn, &job_id, &save_context)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_get_active_jobs(
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<Vec<DispatcherMarketJob>, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_active_jobs(&conn, &save_context)
}

#[command]
pub fn dispatcher_get_job_history(
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherHistoryResponse, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_job_history(&conn, &save_context)
}

#[command]
pub fn dispatcher_get_company_contacts(
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<Vec<DispatcherCompanyContact>, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_company_contacts(&conn, &save_context)
}

#[command]
pub fn dispatcher_create_offer(
    input: DispatcherCreateOfferInput,
    career: State<'_, CareerState>,
) -> Result<DispatcherOffer, String> {
    let runtime = career.runtime.as_ref();
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_create_offer(&conn, input)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_get_offers(
    career: State<'_, CareerState>,
) -> Result<Vec<DispatcherOffer>, String> {
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_offers(&conn)
}

#[command]
pub fn dispatcher_cancel_offer(
    offer_id: String,
    career: State<'_, CareerState>,
) -> Result<DispatcherOffer, String> {
    let runtime = career.runtime.as_ref();
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_cancel_offer(&conn, &offer_id)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_respond_to_counter(
    input: DispatcherRespondToCounterInput,
    career: State<'_, CareerState>,
) -> Result<DispatcherOffer, String> {
    let runtime = career.runtime.as_ref();
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_respond_to_counter(&conn, input)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_get_dispatcher_overview(
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherOverview, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_dispatcher_overview(&conn, &save_context)
}

#[command]
pub fn dispatcher_generate_jobs(
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherGenerationStatus, String> {
    let runtime = career.runtime.as_ref();
    let save_context = resolve_required_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_generate_jobs(&conn, &save_context)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_generate_universal_jobs(
    force: Option<bool>,
    config: Option<DispatcherGenerationConfigInput>,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherGenerationStatus, String> {
    let runtime = career.runtime.as_ref();
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_generate_universal_jobs(
        &conn,
        &save_context,
        force.unwrap_or(false),
        config,
    )?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_get_generation_status(
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherGenerationStatus, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_generation_status(&conn, &save_context)
}

#[command]
pub fn dispatcher_cleanup_expired_jobs(
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherGenerationStatus, String> {
    let runtime = career.runtime.as_ref();
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_cleanup_expired_jobs(&conn, &save_context)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_restore_jobs_for_last_quicksave(
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherGenerationStatus, String> {
    let runtime = career.runtime.as_ref();
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_restore_jobs_for_last_quicksave(&conn, &save_context)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_link_job_to_save_context(
    job_id: String,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherJobDetails, String> {
    let runtime = career.runtime.as_ref();
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_link_job_to_save_context(&conn, &job_id, &save_context)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_assign_job_to_active_save(
    job_id: String,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherJobDetails, String> {
    let runtime = career.runtime.as_ref();
    let save_context = resolve_required_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_assign_job_to_active_save(&conn, &job_id, &save_context)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub async fn dispatcher_assign_and_prepare_ets_link(
    job_id: String,
    app: AppHandle,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
    db: State<'_, EtsDbState>,
) -> Result<DispatcherJobDetails, String> {
    dispatcher_assign_and_prepare_ets_link_inner(
        Some(&app),
        career.runtime.as_ref(),
        profile.inner(),
        &db.pool,
        &job_id,
    )
    .await
}

#[command]
pub async fn dispatcher_assign_and_prepare_and_write(
    job_id: String,
    auto_write: bool,
    app: AppHandle,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
    db: State<'_, EtsDbState>,
) -> Result<DispatcherAssignPrepareWriteDto, String> {
    dispatcher_assign_prepare_write_inner(
        Some(&app),
        career.runtime.as_ref(),
        profile.inner(),
        &db.pool,
        &job_id,
        auto_write,
    )
    .await
}

#[command]
pub fn dispatcher_accept_generated_job(
    job_id: String,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherJobDetails, String> {
    let runtime = career.runtime.as_ref();
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_accept_generated_job(&conn, &job_id, &save_context)?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_get_jobs_for_active_save(
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherJobsBySaveContextResponse, String> {
    let save_context = resolve_required_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_jobs_for_active_save(&conn, &save_context)
}

#[command]
pub fn dispatcher_mark_job_synced_to_ets2(
    job_id: String,
    route_reference: Option<String>,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherJobDetails, String> {
    let runtime = career.runtime.as_ref();
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(runtime)?;
    let result = dispatcher::dispatcher_mark_job_synced_to_ets2(
        &conn,
        &job_id,
        route_reference,
        &save_context,
    )?;
    runtime.overview_dirty.store(true, Ordering::Relaxed);
    Ok(result)
}

#[command]
pub fn dispatcher_get_jobs_by_save_context(
    status: Option<String>,
    career: State<'_, CareerState>,
    profile: State<'_, AppProfileState>,
) -> Result<DispatcherJobsBySaveContextResponse, String> {
    let save_context = resolve_dispatcher_save_context(profile.inner())?;
    let conn = open_connection(career.runtime.as_ref())?;
    dispatcher::dispatcher_get_jobs_by_save_context(&conn, &save_context, status)
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

fn resolve_dispatcher_save_context(
    profile: &AppProfileState,
) -> Result<dispatcher::DispatcherSaveContext, String> {
    snapshot_save_context(profile)
}

fn resolve_required_dispatcher_save_context(
    profile: &AppProfileState,
) -> Result<dispatcher::DispatcherSaveContext, String> {
    let save_context =
        snapshot_save_context(profile).map_err(|_| "save_context_unavailable".to_string())?;
    if save_context.is_ready() {
        Ok(save_context)
    } else {
        Err("no_active_save".to_string())
    }
}

async fn dispatcher_assign_and_prepare_ets_link_inner(
    app: Option<&AppHandle>,
    runtime: &CareerRuntime,
    profile: &AppProfileState,
    db_pool: &SqlitePool,
    job_id: &str,
) -> Result<DispatcherJobDetails, String> {
    let save_context = resolve_required_dispatcher_save_context(profile)?;
    emit_assign_prepare_progress(app, job_id, "assigning");

    let assigned_or_current = {
        let conn = open_connection(runtime)?;
        match dispatcher::dispatcher_assign_job_to_active_save(&conn, job_id, &save_context) {
            Ok(details) => details,
            Err(error) if error == "job_already_assigned" => {
                let current = dispatcher::dispatcher_get_job_by_id(&conn, job_id, &save_context)?;
                if matches!(
                    current.job.status.as_str(),
                    "assigned_to_save" | "prepared" | "injected" | "completed"
                ) {
                    current
                } else {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    };

    runtime.overview_dirty.store(true, Ordering::Relaxed);
    emit_assign_prepare_progress(app, job_id, "preparing");

    let existing_link = match link_service::ets_get_job_link_status(db_pool, job_id).await {
        Ok(link) => Some(link),
        Err(error) if matches!(&error.code, AppErrorCode::InvalidToken) => None,
        Err(error) => {
            let formatted = format_ets_app_error(&error);
            emit_assign_prepare_error(app, job_id, &formatted);
            return Err(formatted);
        }
    };

    if let Some(link) = existing_link {
        if matches!(
            link.status,
            EtsJobLinkStatus::Prepared
                | EtsJobLinkStatus::Written
                | EtsJobLinkStatus::RequiresLoad
                | EtsJobLinkStatus::Synced
                | EtsJobLinkStatus::Completed
        ) {
            let details = load_dispatcher_job_details_for_context(runtime, job_id, &save_context)
                .unwrap_or(assigned_or_current);
            emit_dispatcher_job_updated(app, &details);
            return Ok(details);
        }
    }

    match link_service::prepare_job_link(
        app,
        db_pool,
        job_id,
        save_context
            .profile_reference
            .as_deref()
            .unwrap_or_default(),
        profile,
    )
    .await
    {
        Ok(_) => {
            runtime.overview_dirty.store(true, Ordering::Relaxed);
            let details = load_dispatcher_job_details_for_context(runtime, job_id, &save_context)
                .unwrap_or(assigned_or_current);
            emit_dispatcher_job_updated(app, &details);
            Ok(details)
        }
        Err(error) => {
            let _ = link_service::mark_dispatcher_prepare_error(db_pool, job_id, &error).await;
            runtime.overview_dirty.store(true, Ordering::Relaxed);
            let details =
                load_dispatcher_job_details_for_context(runtime, job_id, &save_context).ok();
            if let Some(details) = details.as_ref() {
                emit_dispatcher_job_updated(app, details);
            }
            let formatted = format_ets_app_error(&error);
            emit_assign_prepare_error(app, job_id, &formatted);
            Err(formatted)
        }
    }
}

async fn dispatcher_assign_prepare_write_inner(
    app: Option<&AppHandle>,
    runtime: &CareerRuntime,
    profile: &AppProfileState,
    db_pool: &SqlitePool,
    job_id: &str,
    auto_write: bool,
) -> Result<DispatcherAssignPrepareWriteDto, String> {
    let save_context = resolve_required_dispatcher_save_context(profile)?;
    let mut assign_result = "pending".to_string();
    let mut prepare_result = "pending".to_string();
    let mut write_result = if auto_write {
        "pending".to_string()
    } else {
        "skipped_auto_write_disabled".to_string()
    };
    let mut write_output: Option<DispatcherWriteOutputDto> = None;
    ensure_steam_cloud_disabled(db_pool, save_context.profile_reference.as_deref())
        .await
        .map_err(|error| {
            let formatted = format_ets_app_error(&error);
            emit_dispatcher_job_error(app, job_id, "validation", &formatted);
            formatted
        })?;

    let game_sii_path = save_context
        .save_reference
        .as_ref()
        .map(|save_reference| PathBuf::from(save_reference).join("game.sii"));
    let sha_before = game_sii_path
        .as_deref()
        .and_then(read_file_sha256_hex_opt);

    emit_dispatcher_progress(app, EVT_DISPATCHER_ASSIGN_PROGRESS, job_id, "assigning");

    let assigned_or_current = {
        let conn = open_connection(runtime)?;
        match dispatcher::dispatcher_assign_job_to_active_save(&conn, job_id, &save_context) {
            Ok(details) => {
                assign_result = "assigned".to_string();
                details
            }
            Err(error) if error == "job_already_assigned" => {
                let current = dispatcher::dispatcher_get_job_by_id(&conn, job_id, &save_context)?;
                if matches!(
                    current.job.status.as_str(),
                    "assigned_to_save" | "prepared" | "injected" | "completed"
                ) {
                    assign_result = "already_assigned".to_string();
                    current
                } else {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    };
    runtime.overview_dirty.store(true, Ordering::Relaxed);

    emit_dispatcher_progress(app, EVT_DISPATCHER_PREPARE_PROGRESS, job_id, "preparing");

    let mut link = match link_service::ets_get_job_link_status(db_pool, job_id).await {
        Ok(link) => Some(link),
        Err(error) if matches!(&error.code, AppErrorCode::InvalidToken) => None,
        Err(error) => {
            let formatted = format_ets_app_error(&error);
            emit_dispatcher_job_error(app, job_id, "prepare", &formatted);
            return Err(formatted);
        }
    };

    let prepare_needed = !matches!(
        link.as_ref().map(|item| item.status),
        Some(
            EtsJobLinkStatus::Prepared
                | EtsJobLinkStatus::Written
                | EtsJobLinkStatus::RequiresLoad
                | EtsJobLinkStatus::Synced
                | EtsJobLinkStatus::Completed
        )
    );

    if prepare_needed {
        match link_service::prepare_job_link(
            app,
            db_pool,
            job_id,
            save_context
                .profile_reference
                .as_deref()
                .unwrap_or_default(),
            profile,
        )
        .await
        {
            Ok(prepared_link) => {
                prepare_result = "prepared".to_string();
                link = Some(prepared_link);
            }
            Err(error) => {
                let _ = link_service::mark_dispatcher_prepare_error(db_pool, job_id, &error).await;
                runtime.overview_dirty.store(true, Ordering::Relaxed);
                let details =
                    load_dispatcher_job_details_for_context(runtime, job_id, &save_context).ok();
                if let Some(details) = details.as_ref() {
                    emit_dispatcher_job_updated(app, details);
                }
                let formatted = format_ets_app_error(&error);
                emit_dispatcher_job_error(app, job_id, "prepare", &formatted);
                return Err(formatted);
            }
        }
    } else {
        prepare_result = "already_prepared_or_written".to_string();
    }

    let mut write_attempted = false;
    let mut write_applied = false;
    let mut written_result: Option<EtsJobWriteResult> = None;

    if auto_write {
        write_attempted = true;
        emit_dispatcher_progress(app, EVT_DISPATCHER_WRITE_PROGRESS, job_id, "writing");

        let current_link = link.clone().ok_or_else(|| {
            let message = "invalid_token: Missing prepared link after prepare step".to_string();
            emit_dispatcher_job_error(app, job_id, "write", &message);
            message
        })?;

        match current_link.status {
            EtsJobLinkStatus::RequiresLoad | EtsJobLinkStatus::Synced | EtsJobLinkStatus::Completed => {
                emit_dispatcher_progress(app, EVT_DISPATCHER_WRITE_PROGRESS, job_id, "already_written");
                write_result = "already_written".to_string();
            }
            EtsJobLinkStatus::Prepared | EtsJobLinkStatus::Written => {
                match link_service::write_job_to_quicksave(app, db_pool, &current_link.link_id)
                .await
                {
                    Ok(write_payload) => {
                        write_result = "written".to_string();
                        link = Some(write_payload.link.clone());
                        written_result = Some(write_payload);
                        write_applied = true;
                    }
                    Err(error) => {
                        let formatted = format_ets_app_error(&error);
                        emit_dispatcher_job_error(app, job_id, "write", &formatted);
                        return Err(formatted);
                    }
                }
            }
            EtsJobLinkStatus::Pending | EtsJobLinkStatus::Error => {
                let message = format!(
                    "invalid_job_status: Link {} is not writable from status {}",
                    current_link.link_id,
                    current_link.status.as_db()
                );
                emit_dispatcher_job_error(app, job_id, "write", &message);
                return Err(message);
            }
        }
    }

    runtime.overview_dirty.store(true, Ordering::Relaxed);
    let details =
        load_dispatcher_job_details_for_context(runtime, job_id, &save_context).unwrap_or(assigned_or_current);
    emit_dispatcher_job_updated(app, &details);
    emit_dispatcher_job_updated_new(app, &details);

    let sha_after = if auto_write {
        game_sii_path
            .as_deref()
            .and_then(read_file_sha256_hex_opt)
    } else {
        None
    };
    let sha_changed = match (sha_before.as_ref(), sha_after.as_ref()) {
        (Some(before), Some(after)) => Some(before != after),
        _ => None,
    };

    if let Some(result) = written_result {
        let patch = result.link.patch.clone();
        let origin = format!("{} ({})", details.job.origin_city, details.job.origin_country);
        let destination = format!(
            "{} ({})",
            details.job.destination_city, details.job.destination_country
        );
        write_output = Some(DispatcherWriteOutputDto {
            job_id: details.job.id.clone(),
            save_path: result.save_path.clone(),
            backup_path: result.backup_path.clone(),
            company_pointer: format!(
                "company.volatile.{}.{}",
                result.link.src_company,
                result.link.src_city
            ),
            offer_pointer: result.link.offer_pointer.clone(),
            job_offer_data_pointer: result.link.job_offer_data_pointer.clone(),
            origin,
            destination,
            target_company: Some(patch.target),
            cargo_token: Some(patch.cargo),
            shortest_distance_km: Some(patch.shortest_distance_km),
            expiration_time: Some(patch.expiration_time),
            reward: details.job.total_reward,
            write_mode: result.write_mode,
            before_sha256: result.before_sha256,
            after_sha256: result.after_sha256,
            job_info_updated: result.job_info_updated,
            final_link_status: result.link.status.as_db().to_string(),
        });
    }

    Ok(DispatcherAssignPrepareWriteDto {
        dispatcher_status: details.job.status.clone(),
        ets2_job_link_status: details.job.ets2_job_link_status.clone(),
        job: details,
        link,
        auto_write,
        write_attempted,
        write_applied,
        sha_before,
        sha_after,
        sha_changed,
        save_reference: save_context.save_reference.clone(),
        quicksave_reference: save_context.quicksave_reference.clone(),
        save_session_id: save_context.save_session_id.clone(),
        assign_result,
        prepare_result,
        write_result,
        write_output,
    })
}

async fn ensure_steam_cloud_disabled(
    pool: &SqlitePool,
    profile_reference: Option<&str>,
) -> Result<(), AppError> {
    let Some(profile_reference) = profile_reference.map(str::trim).filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    let steam_cloud_enabled = sqlx::query_scalar::<_, i64>(
        "SELECT steam_cloud_enabled FROM ets_profiles WHERE profile_id = ?1 OR profile_path = ?1 ORDER BY updated_at_utc DESC LIMIT 1",
    )
    .bind(profile_reference)
    .fetch_optional(pool)
    .await?
    .map(|value| value != 0)
    .unwrap_or(false);

    if steam_cloud_enabled {
        return Err(AppError::new(
            AppErrorCode::SteamCloudEnabled,
            format!("Steam Cloud enabled for profile: {}", profile_reference),
        ));
    }

    Ok(())
}

fn read_file_sha256_hex_opt(path: &std::path::Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Some(format!("{:x}", hasher.finalize()))
}

fn emit_dispatcher_progress(app: Option<&AppHandle>, event: &str, job_id: &str, stage: &str) {
    let Some(app) = app else {
        return;
    };

    let _ = app.emit(
        event,
        serde_json::json!({
            "jobId": job_id,
            "stage": stage,
        }),
    );
}

fn emit_dispatcher_job_updated_new(app: Option<&AppHandle>, details: &DispatcherJobDetails) {
    let Some(app) = app else {
        return;
    };

    let _ = app.emit(
        EVT_DISPATCHER_JOB_UPDATED_NEW,
        serde_json::json!({
            "jobId": details.job.id,
            "status": details.job.status,
            "ets2JobLinkStatus": details.job.ets2_job_link_status,
        }),
    );
}

fn emit_dispatcher_job_error(app: Option<&AppHandle>, job_id: &str, stage: &str, error: &str) {
    let Some(app) = app else {
        return;
    };

    let _ = app.emit(
        EVT_DISPATCHER_JOB_ERROR,
        serde_json::json!({
            "jobId": job_id,
            "stage": stage,
            "error": error,
        }),
    );
}

fn load_dispatcher_job_details_for_context(
    runtime: &CareerRuntime,
    job_id: &str,
    save_context: &dispatcher::DispatcherSaveContext,
) -> Result<DispatcherJobDetails, String> {
    let conn = open_connection(runtime)?;
    dispatcher::dispatcher_get_job_by_id(&conn, job_id, save_context)
}

fn format_ets_app_error(error: &AppError) -> String {
    format!("{}: {}", error.code.as_key(), error.message)
}

fn emit_dispatcher_job_updated(app: Option<&AppHandle>, details: &DispatcherJobDetails) {
    let Some(app) = app else {
        return;
    };

    let _ = app.emit(
        EVT_DISPATCHER_JOB_UPDATED,
        serde_json::json!({
            "jobId": details.job.id,
            "status": details.job.status,
            "ets2JobLinkStatus": details.job.ets2_job_link_status,
        }),
    );
}

fn emit_assign_prepare_progress(app: Option<&AppHandle>, job_id: &str, stage: &str) {
    let Some(app) = app else {
        return;
    };

    let _ = app.emit(
        EVT_DISPATCHER_ASSIGN_PREPARE_PROGRESS,
        serde_json::json!({
            "jobId": job_id,
            "stage": stage,
        }),
    );
}

fn emit_assign_prepare_error(app: Option<&AppHandle>, job_id: &str, error: &str) {
    let Some(app) = app else {
        return;
    };

    let _ = app.emit(
        EVT_DISPATCHER_ASSIGN_PREPARE_ERROR,
        serde_json::json!({
            "jobId": job_id,
            "error": error,
        }),
    );
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use chrono::Utc;
    use rusqlite::{Connection, params};
    use sqlx::SqlitePool;
    use uuid::Uuid;

    use super::{
        dispatcher_assign_and_prepare_ets_link_inner, dispatcher_assign_prepare_write_inner,
    };
    use crate::features::career::dispatcher;
    use crate::features::ets2save::link_service;
    use crate::state::{AppProfileState, CareerRuntime};

    const FIXTURE_GAME_SII: &str = "SiiNunit\n{\ncompany : company.volatile.test_company.berlin {\n job_offer: 1\n job_offer[0]: _nameless.offer.001\n}\njob_offer_data : _nameless.offer.001 {\n target: test_company.munich\n expiration_time: 100\n urgency: 1\n shortest_distance_km: 120\n cargo: cargo.old\n company_truck: false\n trailer_variant: original.variant\n trailer_definition: original.trailer\n units_count: 1\n fill_ratio: 1\n trailer_place: 0\n}\n selected_job: old.job.info\n}\n";

    struct TestContext {
        runtime: CareerRuntime,
        profile_state: AppProfileState,
        pool: SqlitePool,
        db_path: PathBuf,
        job_id: String,
    }

    async fn setup_test_context(
        company_id: &str,
        company_name: &str,
        origin_city: &str,
    ) -> TestContext {
        let temp_root =
            std::env::temp_dir().join(format!("dispatcher_assign_prepare_{}", Uuid::new_v4()));
        let profile_path = temp_root.join("profile");
        let quicksave_dir = profile_path.join("save").join("quicksave");
        fs::create_dir_all(&quicksave_dir).unwrap();
        fs::write(quicksave_dir.join("game.sii"), FIXTURE_GAME_SII).unwrap();

        let db_path = temp_root.join("career.sqlite");
        let conn = Connection::open(&db_path).unwrap();
        crate::features::economy::ensure_tables(&conn).unwrap();
        dispatcher::ensure_tables(&conn).unwrap();

        let job_id = format!("dispatcher-test-{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();
        conn.execute(
            r#"
            INSERT INTO dispatcher_jobs (
                id, source_type, company_id, company_name, job_type, cargo_type,
                origin_city, origin_country, destination_city, destination_country,
                distance_km, cargo_mass_kg, urgency_level, difficulty_level,
                equipment_type_required, trailer_type_required, base_rate_per_km,
                calculated_rate_per_km, total_reward, estimated_duration_minutes,
                payment_tier_snapshot, payment_multiplier_snapshot, country_multiplier_snapshot,
                reputation_multiplier_snapshot, cargo_multiplier_snapshot,
                urgency_multiplier_snapshot, equipment_multiplier_snapshot,
                market_variation_snapshot, customer_multiplier_snapshot, company_reputation,
                fuel_cost_estimate, profit_estimate, risk_note, bonus_note,
                expires_at_utc, status, progress_km, profile_reference, save_reference,
                quicksave_reference, save_session_id, route_reference, ets2_job_link_status,
                last_error_code, last_error_message, accepted_at_utc, completed_at_utc,
                created_at_utc, updated_at_utc
            )
            VALUES (
                ?1, 'generated', ?2, ?3, 'quick_job', 'trucks',
                ?4, 'DE', 'Hamburg', 'DE',
                520.0, 12000.0, 'normal', 'normal',
                'quick_job', NULL, 1.12,
                1.18, 758, 360,
                'standard', 1.0, 1.02,
                1.01, 1.0,
                1.0, 1.0,
                1.0, 1.0, 320,
                120, 480, NULL, NULL,
                NULL, 'open', 0, NULL, NULL,
                NULL, NULL, NULL, 'pending_route',
                NULL, NULL, NULL, NULL, ?5, ?5
            )
            "#,
            params![job_id, company_id, company_name, origin_city, now],
        )
        .unwrap();
        drop(conn);

        let pool = link_service::create_pool(&db_path).await.unwrap();
        let runtime = CareerRuntime::default();
        *runtime.db_path.lock().unwrap() = Some(db_path.clone());

        let profile_state = AppProfileState::default();
        *profile_state.current_profile.lock().unwrap() = Some(profile_path.display().to_string());
        *profile_state.current_save.lock().unwrap() = Some(quicksave_dir.display().to_string());
        *profile_state.selected_game.lock().unwrap() = "ets2".to_string();

        TestContext {
            runtime,
            profile_state,
            pool,
            db_path,
            job_id,
        }
    }

    #[test]
    fn assign_and_prepare_sets_status_prepared() {
        tauri::async_runtime::block_on(async {
            let context = setup_test_context("test_company", "Test Company", "Berlin").await;

            let result = dispatcher_assign_and_prepare_ets_link_inner(
                None,
                &context.runtime,
                &context.profile_state,
                &context.pool,
                &context.job_id,
            )
            .await
            .unwrap();

            assert_eq!(result.job.status, "prepared");
            assert_eq!(result.job.ets2_job_link_status.as_deref(), Some("prepared"));

            let conn = Connection::open(&context.db_path).unwrap();
            let link_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM ets_job_links WHERE vtc_job_id = ?1",
                    [context.job_id.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(link_count, 1);
        });
    }

    #[test]
    fn prepare_failure_leaves_job_assigned_and_marks_error() {
        tauri::async_runtime::block_on(async {
            let context =
                setup_test_context("missing_company", "Missing Company", "UnknownCity").await;

            let error = dispatcher_assign_and_prepare_ets_link_inner(
                None,
                &context.runtime,
                &context.profile_state,
                &context.pool,
                &context.job_id,
            )
            .await
            .unwrap_err();

            assert!(error.contains("company_not_found_in_save"));

            let conn = Connection::open(&context.db_path).unwrap();
            let row = conn
                .query_row(
                    "SELECT status, ets2_job_link_status, last_error_code FROM dispatcher_jobs WHERE id = ?1",
                    [context.job_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(row.0, "assigned_to_save");
            assert_eq!(row.1.as_deref(), Some("error"));
            assert_eq!(row.2.as_deref(), Some("company_not_found_in_save"));
        });
    }

    #[test]
    fn assign_and_prepare_is_idempotent() {
        tauri::async_runtime::block_on(async {
            let context = setup_test_context("test_company", "Test Company", "Berlin").await;

            let first = dispatcher_assign_and_prepare_ets_link_inner(
                None,
                &context.runtime,
                &context.profile_state,
                &context.pool,
                &context.job_id,
            )
            .await
            .unwrap();
            let second = dispatcher_assign_and_prepare_ets_link_inner(
                None,
                &context.runtime,
                &context.profile_state,
                &context.pool,
                &context.job_id,
            )
            .await
            .unwrap();

            assert_eq!(first.job.status, "prepared");
            assert_eq!(second.job.status, "prepared");

            let conn = Connection::open(&context.db_path).unwrap();
            let link_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM ets_job_links WHERE vtc_job_id = ?1",
                    [context.job_id.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(link_count, 1);
        });
    }

    #[test]
    fn prepare_resolves_tokens_for_missing_company_against_save_depots() {
        tauri::async_runtime::block_on(async {
            let context =
                setup_test_context("north-axis-logistics", "North Axis Logistics", "Berlin").await;

            let result = dispatcher_assign_and_prepare_ets_link_inner(
                None,
                &context.runtime,
                &context.profile_state,
                &context.pool,
                &context.job_id,
            )
            .await
            .unwrap();

            assert_eq!(result.job.status, "prepared");
            assert_eq!(result.job.ets2_job_link_status.as_deref(), Some("prepared"));

            let conn = Connection::open(&context.db_path).unwrap();
            let row = conn
                .query_row(
                    "SELECT resolved_source_company_token, resolved_source_city_token, resolved_target_company_token, resolved_target_city_token FROM ets_job_links WHERE vtc_job_id = ?1",
                    [context.job_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(row.1.as_deref(), Some("berlin"));
            assert!(row.0.as_deref().is_some_and(|value| !value.is_empty()));
            assert!(row.2.as_deref().is_some_and(|value| !value.is_empty()));
            assert!(row.3.as_deref().is_some_and(|value| !value.is_empty()));
        });
    }

    #[test]
    fn assign_prepare_write_sets_requires_load_and_injected() {
        tauri::async_runtime::block_on(async {
            let context = setup_test_context("test_company", "Test Company", "Berlin").await;

            let result = dispatcher_assign_prepare_write_inner(
                None,
                &context.runtime,
                &context.profile_state,
                &context.pool,
                &context.job_id,
                true,
            )
            .await
            .unwrap();

            assert_eq!(result.dispatcher_status, "injected");
            assert_eq!(result.ets2_job_link_status.as_deref(), Some("requires_load"));
            assert!(result.write_attempted);

            let conn = Connection::open(&context.db_path).unwrap();
            let row = conn
                .query_row(
                    "SELECT status, ets2_job_link_status FROM dispatcher_jobs WHERE id = ?1",
                    [context.job_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(row.0, "injected");
            assert_eq!(row.1.as_deref(), Some("requires_load"));

            let save_path = context
                .profile_state
                .current_save
                .lock()
                .unwrap()
                .clone()
                .unwrap();
            let written = std::fs::read_to_string(format!("{}/game.sii", save_path)).unwrap();
            assert!(written.contains("cargo: cargo.trucks"));
            assert!(written.contains("target: test_company.hamburg"));
            assert!(written.contains("shortest_distance_km: 520"));
        });
    }

    #[test]
    fn assign_prepare_write_is_idempotent_when_already_written() {
        tauri::async_runtime::block_on(async {
            let context = setup_test_context("test_company", "Test Company", "Berlin").await;

            let first = dispatcher_assign_prepare_write_inner(
                None,
                &context.runtime,
                &context.profile_state,
                &context.pool,
                &context.job_id,
                true,
            )
            .await
            .unwrap();
            let second = dispatcher_assign_prepare_write_inner(
                None,
                &context.runtime,
                &context.profile_state,
                &context.pool,
                &context.job_id,
                true,
            )
            .await
            .unwrap();

            assert_eq!(first.dispatcher_status, "injected");
            assert_eq!(second.dispatcher_status, "injected");

            let conn = Connection::open(&context.db_path).unwrap();
            let link_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM ets_job_links WHERE vtc_job_id = ?1",
                    [context.job_id.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(link_count, 1);
        });
    }
}
