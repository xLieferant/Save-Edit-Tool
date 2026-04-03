use rusqlite::Connection;

use crate::features::auth::repo as auth_repo;
use crate::features::companies::models::{Company, CompanyListItem, NewCompany};
use crate::features::companies::repo;
use crate::state::AuthState;

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn require_user_id(auth: &AuthState) -> Result<i64, String> {
    let session = auth
        .session
        .lock()
        .map_err(|_| "AuthState session lock poisoned".to_string())?
        .clone();
    let Some(session) = session else {
        return Err("Not authenticated".to_string());
    };
    Ok(session.user_id)
}

pub fn list_companies(conn: &Connection, limit: i64) -> Result<Vec<CompanyListItem>, String> {
    repo::list_companies(conn, limit)
}

pub fn create_company(
    conn: &mut Connection,
    auth: &AuthState,
    name: String,
    logo_path: Option<String>,
    description: Option<String>,
    salary_base: i64,
    location: String,
    job_type: String,
) -> Result<Company, String> {
    let user_id = require_user_id(auth)?;
    if name.trim().is_empty() {
        return Err("Company name is required".to_string());
    }
    if location.trim().is_empty() {
        return Err("Company location is required".to_string());
    }
    if job_type.trim().is_empty() {
        return Err("Job type is required".to_string());
    }
    if salary_base < 0 {
        return Err("Salary must be a non-negative number".to_string());
    }

    let user = auth_repo::load_user_by_id(conn, user_id)?
        .ok_or_else(|| "User not found".to_string())?;
    if let Some(company_id) = user.company_id {
        if repo::load_company_by_id(conn, company_id)?.is_none() {
            auth_repo::clear_user_company(conn, user_id)?;
        } else {
            return Err("User is already in a company".to_string());
        }
    }

    let now = now_rfc3339();
    let company = NewCompany {
        owner_user_id: user_id,
        name: name.trim().to_string(),
        logo_path: logo_path.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        }),
        logo_blob: None,
        logo_mime: None,
        header_path: None,
        header_blob: None,
        header_mime: None,
        language: None,
        game: None,
        description: description.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        }),
        salary_base,
        location: location.trim().to_string(),
        job_type: job_type.trim().to_string(),
        created_at: now.clone(),
        updated_at: now.clone(),
        is_active: true,
    };

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let company_id = repo::insert_company(&tx, &company)?;
    repo::insert_member(&tx, company_id, user_id, "owner", &now)?;
    auth_repo::update_user_company(&tx, user_id, company_id)?;
    tx.commit().map_err(|e| e.to_string())?;

    repo::load_company_by_id(conn, company_id)?
        .ok_or_else(|| "Failed to load created company".to_string())
}

pub fn create_company_onboarding(
    conn: &mut Connection,
    auth: &AuthState,
    name: String,
    location: String,
    language: String,
    game: String,
    description: Option<String>,
    logo_blob: Option<Vec<u8>>,
    logo_mime: Option<String>,
    header_blob: Option<Vec<u8>>,
    header_mime: Option<String>,
) -> Result<Company, String> {
    let user_id = require_user_id(auth)?;

    if name.trim().is_empty() {
        return Err("Company name is required".to_string());
    }
    if location.trim().is_empty() {
        return Err("Company location is required".to_string());
    }
    if language.trim().is_empty() {
        return Err("Company language is required".to_string());
    }

    let normalized_game = game.trim().to_uppercase();
    if normalized_game != "ETS2" && normalized_game != "ATS" {
        return Err("Game must be ETS2 or ATS".to_string());
    }

    if repo::find_company_id_by_name(conn, name.trim())?.is_some() {
        return Err("Company name already exists".to_string());
    }

    let user = auth_repo::load_user_by_id(conn, user_id)?
        .ok_or_else(|| "User not found".to_string())?;
    if let Some(company_id) = user.company_id {
        if repo::load_company_by_id(conn, company_id)?.is_none() {
            auth_repo::clear_user_company(conn, user_id)?;
        } else {
            return Err("User is already in a company".to_string());
        }
    }

    let now = now_rfc3339();
    let company = NewCompany {
        owner_user_id: user_id,
        name: name.trim().to_string(),
        logo_path: None,
        logo_blob,
        logo_mime: logo_mime.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        }),
        header_path: None,
        header_blob,
        header_mime: header_mime.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        }),
        language: Some(language.trim().to_string()),
        game: Some(normalized_game),
        description: description.and_then(|v| {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        }),
        salary_base: 0,
        location: location.trim().to_string(),
        job_type: "default".to_string(),
        created_at: now.clone(),
        updated_at: now.clone(),
        is_active: true,
    };

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let company_id = repo::insert_company(&tx, &company)?;
    repo::insert_member(&tx, company_id, user_id, "owner", &now)?;
    auth_repo::update_user_company(&tx, user_id, company_id)?;
    tx.commit().map_err(|e| e.to_string())?;

    repo::load_company_by_id(conn, company_id)?
        .ok_or_else(|| "Failed to load created company".to_string())
}

pub fn join_company(conn: &mut Connection, auth: &AuthState, company_id: i64) -> Result<Company, String> {
    // MVP: Direct join without invitations / approvals.
    let user_id = require_user_id(auth)?;
    let user = auth_repo::load_user_by_id(conn, user_id)?
        .ok_or_else(|| "User not found".to_string())?;
    if let Some(existing_company_id) = user.company_id {
        if repo::load_company_by_id(conn, existing_company_id)?.is_none() {
            auth_repo::clear_user_company(conn, user_id)?;
        } else {
            return Err("User is already in a company".to_string());
        }
    }

    let company = repo::load_company_by_id(conn, company_id)?
        .ok_or_else(|| "Company not found".to_string())?;
    if !company.is_active {
        return Err("Company is inactive".to_string());
    }

    let now = now_rfc3339();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if repo::is_user_member_of_company(&tx, company_id, user_id)? {
        return Err("User is already a member".to_string());
    }
    repo::insert_member(&tx, company_id, user_id, "member", &now)?;
    auth_repo::update_user_company(&tx, user_id, company_id)?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(company)
}

pub fn get_current_company(conn: &Connection, auth: &AuthState) -> Result<Option<Company>, String> {
    let user_id = match require_user_id(auth) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let user = match auth_repo::load_user_by_id(conn, user_id)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let Some(company_id) = user.company_id else {
        return Ok(None);
    };
    repo::load_company_by_id(conn, company_id)
}

pub fn get_company_for_user(conn: &Connection, auth: &AuthState, user_id: i64) -> Result<Option<Company>, String> {
    let current_user_id = require_user_id(auth)?;
    if current_user_id != user_id {
        return Err("Forbidden".to_string());
    }

    let user = auth_repo::load_user_by_id(conn, user_id)?;
    let Some(user) = user else {
        return Ok(None);
    };
    let Some(company_id) = user.company_id else {
        return Ok(None);
    };
    repo::load_company_by_id(conn, company_id)
}
