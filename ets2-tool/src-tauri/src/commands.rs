use rusqlite::Connection;
use tauri::{AppHandle, State, command};

use crate::features::ets2save::errors::AppError;
use crate::features::ets2save::link_service;
use crate::features::ets2save::models::{EtsJobLink, EtsSaveSlot};
use crate::shared::ets2data;
use crate::shared::ets2data::import;
use crate::shared::ets2data::models::{CityQueryFilter, CityRecord, CompanyRecord, Ets2DataImportSummary};
use crate::state::{AppProfileState, CareerState, EtsDbState};

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
) -> Result<EtsJobLink, AppError> {
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
pub fn data_import_ets2_datasets(
    force: bool,
    app: AppHandle,
    career: State<'_, CareerState>,
) -> Result<Ets2DataImportSummary, String> {
    let mut conn = open_runtime_db(career.inner())?;
    import::import_datasets_with_error_event(
        &app,
        &mut conn,
        &ets2data::default_repo_root(),
        force,
    )
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
