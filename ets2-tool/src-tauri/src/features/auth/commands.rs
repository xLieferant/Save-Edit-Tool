use rusqlite::Connection;
use tauri::command;
use tauri::State;

use crate::features::auth::db;
use crate::features::auth::models::{AuthAccountOverview, AuthAdminDbOverview, AuthLoginResult, AuthRegisterResult, PublicUser};
use crate::features::auth::service;
use crate::state::AuthState;

#[command]
pub fn auth_seed_default_admin() -> Result<(), String> {
    let db_path = db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    db::ensure_tables(&conn)?;
    service::seed_default_admin(&conn)?;
    Ok(())
}

#[command]
pub fn auth_register(
    username: String,
    email: String,
    password: String,
    password_confirm: String,
    consent_privacy: bool,
    consent_terms: bool,
    remember_me: bool,
    auth: State<'_, AuthState>,
) -> Result<AuthRegisterResult, String> {
    let db_path = db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    db::ensure_tables(&conn)?;
    service::seed_default_admin(&conn)?;

    let (user, remember_me) = service::register_local(
        &conn,
        auth.inner(),
        username,
        email,
        password,
        password_confirm,
        consent_privacy,
        consent_terms,
        remember_me,
    )?;

    Ok(AuthRegisterResult { user, remember_me })
}

#[command]
pub fn auth_login(
    email: String,
    password: String,
    remember_me: bool,
    auth: State<'_, AuthState>,
) -> Result<AuthLoginResult, String> {
    let db_path = db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    db::ensure_tables(&conn)?;
    service::seed_default_admin(&conn)?;

    let (user, remember_me) =
        service::login_local(&conn, auth.inner(), email, password, remember_me)?;
    Ok(AuthLoginResult { user, remember_me })
}

#[command]
pub fn auth_logout(auth: State<'_, AuthState>) -> Result<(), String> {
    service::logout_local(auth.inner())
}

#[command]
pub fn auth_get_current_user(auth: State<'_, AuthState>) -> Result<Option<PublicUser>, String> {
    let db_path = db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    db::ensure_tables(&conn)?;
    service::seed_default_admin(&conn)?;
    service::get_current_user(&conn, auth.inner())
}

#[command]
pub fn auth_restore_session(auth: State<'_, AuthState>) -> Result<(), String> {
    let db_path = db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    db::ensure_tables(&conn)?;
    service::seed_default_admin(&conn)?;
    service::restore_persisted_session(&conn, auth.inner())
}

#[command]
pub fn auth_get_account_overview(auth: State<'_, AuthState>) -> Result<AuthAccountOverview, String> {
    let db_path = db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    db::ensure_tables(&conn)?;
    service::seed_default_admin(&conn)?;
    service::get_account_overview(&conn, auth.inner())
}

#[command]
pub fn auth_generate_recovery_codes(auth: State<'_, AuthState>) -> Result<Vec<String>, String> {
    let db_path = db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    db::ensure_tables(&conn)?;
    service::seed_default_admin(&conn)?;
    service::generate_recovery_codes(&conn, auth.inner(), 5)
}

#[command]
pub fn auth_reset_password_with_recovery_code(
    email: String,
    recovery_code: String,
    new_password: String,
    new_password_confirm: String,
) -> Result<(), String> {
    let db_path = db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    db::ensure_tables(&conn)?;
    service::seed_default_admin(&conn)?;
    service::reset_password_with_recovery_code(
        &conn,
        email,
        recovery_code,
        new_password,
        new_password_confirm,
    )
}

#[command]
pub fn auth_admin_get_db_overview(auth: State<'_, AuthState>) -> Result<AuthAdminDbOverview, String> {
    let db_path = db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    db::ensure_tables(&conn)?;
    service::seed_default_admin(&conn)?;
    service::admin_get_db_overview(&conn, auth.inner())
}
