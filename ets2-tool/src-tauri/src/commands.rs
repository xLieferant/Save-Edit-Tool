use tauri::{AppHandle, State, command};

use crate::features::ets2save::errors::AppError;
use crate::features::ets2save::link_service;
use crate::features::ets2save::models::{EtsJobLink, EtsSaveSlot};
use crate::state::{AppProfileState, EtsDbState};

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
