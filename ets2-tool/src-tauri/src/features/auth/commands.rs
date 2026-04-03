use rusqlite::Connection;
use tauri::command;
use tauri::State;

use crate::features::auth::db;
use crate::features::auth::models::{AuthLoginResult, AuthRegisterResult, PublicUser};
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
