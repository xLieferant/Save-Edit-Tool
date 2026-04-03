use rusqlite::Connection;

use crate::features::auth::repo as auth_repo;
use crate::features::companies::repo as company_repo;
use crate::state::AuthState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CareerOnboardingState {
    pub needs_login: bool,
    pub needs_company: bool,
    pub has_company: bool,
    pub company_id: Option<i64>,
    pub is_company_owner: bool,
}

pub fn get_onboarding_state(
    conn: &Connection,
    auth: &AuthState,
) -> Result<CareerOnboardingState, String> {
    let session = auth
        .session
        .lock()
        .map_err(|_| "AuthState session lock poisoned".to_string())?
        .clone();
    let Some(session) = session else {
        return Ok(CareerOnboardingState {
            needs_login: true,
            needs_company: false,
            has_company: false,
            company_id: None,
            is_company_owner: false,
        });
    };

    let user = auth_repo::load_user_by_id(conn, session.user_id)?
        .ok_or_else(|| "User not found".to_string())?;
    if !user.is_active {
        return Ok(CareerOnboardingState {
            needs_login: true,
            needs_company: false,
            has_company: false,
            company_id: None,
            is_company_owner: false,
        });
    }

    let Some(company_id) = user.company_id else {
        return Ok(CareerOnboardingState {
            needs_login: false,
            needs_company: true,
            has_company: false,
            company_id: None,
            is_company_owner: false,
        });
    };

    let company = company_repo::load_company_by_id(conn, company_id)?;
    let Some(company) = company else {
        return Ok(CareerOnboardingState {
            needs_login: false,
            needs_company: true,
            has_company: false,
            company_id: None,
            is_company_owner: false,
        });
    };

    Ok(CareerOnboardingState {
        needs_login: false,
        needs_company: false,
        has_company: true,
        company_id: Some(company.id),
        is_company_owner: company.owner_user_id == user.id,
    })
}

