use rusqlite::Connection;
use tauri::command;
use tauri::State;

use crate::features::auth::db as auth_db;
use crate::features::auth::service as auth_service;
use crate::features::companies::db;
use crate::features::companies::models::{Company, CompanyListItem};
use crate::features::companies::service;
use crate::state::AuthState;
use base64::Engine;

#[command]
pub fn company_list(limit: Option<i64>) -> Result<Vec<CompanyListItem>, String> {
    let db_path = auth_db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    auth_db::ensure_tables(&conn)?;
    db::ensure_tables(&conn)?;
    auth_service::seed_default_admin(&conn)?;

    service::list_companies(&conn, limit.unwrap_or(50))
}

#[command]
pub fn company_create(
    name: String,
    logo_path: Option<String>,
    description: Option<String>,
    salary_base: i64,
    location: String,
    job_type: String,
    auth: State<'_, AuthState>,
) -> Result<Company, String> {
    let db_path = auth_db::default_db_path();
    let mut conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    auth_db::ensure_tables(&conn)?;
    db::ensure_tables(&conn)?;
    auth_service::seed_default_admin(&conn)?;

    service::create_company(
        &mut conn,
        auth.inner(),
        name,
        logo_path,
        description,
        salary_base,
        location,
        job_type,
    )
}

#[command]
pub fn company_create_onboarding(
    name: String,
    location: String,
    language: String,
    game: String,
    description: Option<String>,
    logo_base64: Option<String>,
    logo_mime: Option<String>,
    header_base64: Option<String>,
    header_mime: Option<String>,
    auth: State<'_, AuthState>,
) -> Result<Company, String> {
    let db_path = auth_db::default_db_path();
    let mut conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    auth_db::ensure_tables(&conn)?;
    db::ensure_tables(&conn)?;
    auth_service::seed_default_admin(&conn)?;

    let logo_blob = match logo_base64 {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(
                    base64::engine::general_purpose::STANDARD
                        .decode(trimmed)
                        .map_err(|e| e.to_string())?,
                )
            }
        }
        None => None,
    };

    let header_blob = match header_base64 {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(
                    base64::engine::general_purpose::STANDARD
                        .decode(trimmed)
                        .map_err(|e| e.to_string())?,
                )
            }
        }
        None => None,
    };

    service::create_company_onboarding(
        &mut conn,
        auth.inner(),
        name,
        location,
        language,
        game,
        description,
        logo_blob,
        logo_mime,
        header_blob,
        header_mime,
    )
}

#[command]
pub fn company_join(company_id: i64, auth: State<'_, AuthState>) -> Result<Company, String> {
    let db_path = auth_db::default_db_path();
    let mut conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    auth_db::ensure_tables(&conn)?;
    db::ensure_tables(&conn)?;
    auth_service::seed_default_admin(&conn)?;

    service::join_company(&mut conn, auth.inner(), company_id)
}

#[command]
pub fn company_get_current(auth: State<'_, AuthState>) -> Result<Option<Company>, String> {
    let db_path = auth_db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    auth_db::ensure_tables(&conn)?;
    db::ensure_tables(&conn)?;
    auth_service::seed_default_admin(&conn)?;

    service::get_current_company(&conn, auth.inner())
}

#[command]
pub fn company_get_for_user(user_id: i64, auth: State<'_, AuthState>) -> Result<Option<Company>, String> {
    let db_path = auth_db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    auth_db::ensure_tables(&conn)?;
    db::ensure_tables(&conn)?;
    auth_service::seed_default_admin(&conn)?;

    service::get_company_for_user(&conn, auth.inner(), user_id)
}
