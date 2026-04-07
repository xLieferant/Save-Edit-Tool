use rusqlite::Connection;
use tauri::{AppHandle, State, command};

use crate::features::ets2save::errors::AppError;
use crate::features::ets2save::link_service;
use crate::features::ets2save::models::{EtsJobLink, EtsJobWriteResult, EtsSaveSlot};
use crate::features::ets2save::snapshot::{self, SaveSnapshotInput};
use crate::shared::current_profile::snapshot_save_context;
use crate::shared::ets2data;
use crate::shared::ets2data::import;
use crate::shared::ets2data::models::{
    CityQueryFilter, CityRecord, CompanyRecord, Ets2DataImportSummary,
};
use crate::state::{AppProfileState, AppState, CareerState, EtsDbState};

#[command]
pub async fn ets_get_last_quicksave(
    profile_id: String,
    profile_state: State<'_, AppProfileState>,
    db: State<'_, EtsDbState>,
) -> Result<EtsSaveSlot, AppError> {
    link_service::ets_get_last_quicksave(&db.pool, &profile_id, profile_state.inner()).await
}

#[command]
pub async fn ets_prepare_job_link(
    vtc_job_id: String,
    profile_id: String,
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    db: State<'_, EtsDbState>,
) -> Result<EtsJobLink, AppError> {
    link_service::ets_prepare_job_link(
        &app,
        &db.pool,
        &vtc_job_id,
        &profile_id,
        profile_state.inner(),
    )
    .await
}

#[command]
pub async fn ets_write_job_to_quicksave(
    link_id: String,
    app: AppHandle,
    db: State<'_, EtsDbState>,
) -> Result<EtsJobWriteResult, AppError> {
    link_service::ets_write_job_to_quicksave(&app, &db.pool, &link_id).await
}

#[command]
pub async fn ets_get_job_link_status(
    vtc_job_id: String,
    db: State<'_, EtsDbState>,
) -> Result<EtsJobLink, AppError> {
    link_service::ets_get_job_link_status(&db.pool, &vtc_job_id).await
}

#[command]
pub async fn ets_snapshot_refresh_active_save(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
    db: State<'_, EtsDbState>,
) -> Result<snapshot::SaveSnapshotDto, AppError> {
    let context = snapshot_save_context(profile_state.inner()).map_err(|error| {
        AppError::new(
            crate::features::ets2save::errors::AppErrorCode::InvalidToken,
            error,
        )
    })?;
    let save_session_id = context.save_session_id.clone().ok_or_else(|| {
        AppError::new(
            crate::features::ets2save::errors::AppErrorCode::InvalidToken,
            "save_session_id missing",
        )
    })?;

    let input = SaveSnapshotInput {
        save_session_id,
        profile_reference: context.profile_reference.clone(),
        save_reference: context.save_reference.clone(),
        quicksave_reference: context.quicksave_reference.clone(),
    };

    snapshot::snapshot_refresh(Some(&app), &db.pool, input)
        .await
        .map_err(|error| {
            snapshot::emit_error(
                Some(&app),
                context.save_session_id.as_deref().unwrap_or(""),
                &error.to_string(),
            );
            error
        })
}

#[command]
pub async fn ets_snapshot_get_active(
    profile_state: State<'_, AppProfileState>,
    db: State<'_, EtsDbState>,
) -> Result<Option<snapshot::SaveSnapshotDto>, AppError> {
    let context = snapshot_save_context(profile_state.inner()).map_err(|error| {
        AppError::new(
            crate::features::ets2save::errors::AppErrorCode::InvalidToken,
            error,
        )
    })?;
    let Some(save_session_id) = context.save_session_id.as_deref() else {
        return Ok(None);
    };
    snapshot::snapshot_get_by_session(&db.pool, save_session_id).await
}

#[command]
pub async fn ets_snapshot_list_depots(
    city_token: Option<String>,
    profile_state: State<'_, AppProfileState>,
    db: State<'_, EtsDbState>,
) -> Result<Vec<snapshot::SaveSnapshotDepotDto>, AppError> {
    let context = snapshot_save_context(profile_state.inner()).map_err(|error| {
        AppError::new(
            crate::features::ets2save::errors::AppErrorCode::InvalidToken,
            error,
        )
    })?;
    let save_session_id = context.save_session_id.clone().ok_or_else(|| {
        AppError::new(
            crate::features::ets2save::errors::AppErrorCode::InvalidToken,
            "save_session_id missing",
        )
    })?;
    snapshot::snapshot_list_depots(&db.pool, &save_session_id, city_token.as_deref()).await
}

#[command]
pub async fn ets_snapshot_get_active_diagnostics(
    profile_state: State<'_, AppProfileState>,
    db: State<'_, EtsDbState>,
) -> Result<Option<snapshot::SaveSnapshotDiagnosticsDto>, AppError> {
    let context = snapshot_save_context(profile_state.inner()).map_err(|error| {
        AppError::new(
            crate::features::ets2save::errors::AppErrorCode::InvalidToken,
            error,
        )
    })?;
    let Some(save_session_id) = context.save_session_id.as_deref() else {
        return Ok(None);
    };
    snapshot::snapshot_diagnostics_by_session(&db.pool, save_session_id).await
}

#[command]
pub async fn get_sqlite_info(
    app_state: State<'_, AppState>,
) -> Result<crate::db::sqlite::SqliteInfoDto, String> {
    crate::db::sqlite::get_sqlite_info(&app_state.sqlite).await
}

#[command]
pub async fn get_sqlite_table_counts(
    profile_state: State<'_, AppProfileState>,
    app_state: State<'_, AppState>,
) -> Result<crate::db::sqlite::SqliteTableCountsDto, String> {
    let save_context = snapshot_save_context(profile_state.inner()).unwrap_or_default();
    let counts =
        crate::db::sqlite::get_sqlite_table_counts(&app_state.sqlite, save_context.save_session_id)
            .await?;
    crate::dev_log!(
        "[db] table counts session={:?} ets2_companies={} ets_save_snapshot={} ets_save_depots={} ets_save_visited_cities={} ets_save_transport_cargo={} ets_save_snapshot_meta={} ets_job_links={} dispatcher_jobs={} vtc_companies={} vtc_company_members={} vtc_local_context={}",
        counts.active_save_session_id,
        counts.ets2_companies,
        counts.ets_save_snapshot,
        counts.ets_save_depots,
        counts.ets_save_visited_cities,
        counts.ets_save_transport_cargo,
        counts.ets_save_snapshot_meta,
        counts.ets_job_links,
        counts.dispatcher_jobs,
        counts.vtc_companies,
        counts.vtc_company_members,
        counts.vtc_local_context
    );
    Ok(counts)
}

#[command]
pub fn data_import_ets2_datasets(
    force: bool,
    app: AppHandle,
    career: State<'_, CareerState>,
) -> Result<Ets2DataImportSummary, String> {
    let mut conn = open_runtime_db(career.inner())?;
    import::import_datasets_with_error_event(&app, &mut conn, &ets2data::default_repo_root(), force)
}

#[command]
pub fn ets2data_get_city(
    city_id: String,
    career: State<'_, CareerState>,
) -> Result<Option<CityRecord>, String> {
    let conn = open_runtime_db(career.inner())?;
    import::get_city(&conn, &city_id)
}

#[command]
pub fn ets2data_get_company(
    company_id: String,
    career: State<'_, CareerState>,
) -> Result<Option<CompanyRecord>, String> {
    let conn = open_runtime_db(career.inner())?;
    import::get_company(&conn, &company_id)
}

#[command]
pub fn ets2data_list_cities(
    filters: Option<CityQueryFilter>,
    career: State<'_, CareerState>,
) -> Result<Vec<CityRecord>, String> {
    let conn = open_runtime_db(career.inner())?;
    import::list_cities(&conn, filters)
}

fn open_runtime_db(career: &CareerState) -> Result<Connection, String> {
    let db_path = career
        .runtime
        .db_path
        .lock()
        .map_err(|_| "Career db_path lock poisoned".to_string())?
        .clone()
        .ok_or_else(|| "Career database path not initialized".to_string())?;
    Connection::open(db_path).map_err(|error| error.to_string())
}
