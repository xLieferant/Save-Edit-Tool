use rusqlite::Connection;
use tauri::command;
use tauri::State;

use crate::features::auth::db as auth_db;
use crate::features::auth::service as auth_service;
use crate::features::companies::db as company_db;
use crate::features::career_onboarding::service::{self, CareerOnboardingState};
use crate::state::AuthState;

#[command]
pub fn career_get_onboarding_state(auth: State<'_, AuthState>) -> Result<CareerOnboardingState, String> {
    let db_path = auth_db::default_db_path();
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    auth_db::ensure_tables(&conn)?;
    company_db::ensure_tables(&conn)?;
    auth_service::seed_default_admin(&conn)?;

    service::get_onboarding_state(&conn, auth.inner())
}
