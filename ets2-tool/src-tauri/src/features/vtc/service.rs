use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;

use crate::features::auth::repo as auth_repo;
use crate::features::auth::service as auth_service;
use crate::features::vtc::models::{
    CareerSettings, CompanyMember, CompanyOverview, CompanyRoleOption, CompanySettings,
    CreateCompanyInput, UpdateCareerSettingsInput, UpdateCompanyProfileInput,
    UpdateCompanySettingsInput, UpdateUserProfileMetaInput, UpdateUserSettingsInput, UserProfile,
    UserSettings, UsernameAvailability,
};
use crate::features::vtc::repo;
use crate::state::AuthState;

const USERNAME_COOLDOWN_DAYS: i64 = 14;

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn require_user_id(auth: &AuthState) -> Result<i64, String> {
    let session = auth
        .session
        .lock()
        .map_err(|_| "not_allowed".to_string())?
        .clone();
    let Some(session) = session else {
        return Err("not_allowed".to_string());
    };
    Ok(session.user_id)
}

fn normalize_required(value: &str) -> String {
    value.trim().to_string()
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn validate_username_format(username: &str) -> Result<String, String> {
    let normalized = normalize_required(username);
    if normalized.len() < 3 {
        return Err("invalid_username".to_string());
    }
    Ok(normalized)
}

fn validate_game(game: &str) -> Result<String, String> {
    let normalized = normalize_required(game).to_uppercase();
    if normalized != "ETS2" && normalized != "ATS" {
        return Err("invalid_game".to_string());
    }
    Ok(normalized)
}

fn compute_next_username_change(last_changed: Option<&str>) -> Option<String> {
    let Some(last_changed) = last_changed else {
        return None;
    };
    let parsed = DateTime::parse_from_rfc3339(last_changed).ok()?;
    let next = parsed.with_timezone(&Utc) + Duration::days(USERNAME_COOLDOWN_DAYS);
    Some(next.to_rfc3339())
}

fn enforce_company_access(
    conn: &Connection,
    user_id: i64,
) -> Result<(i64, Option<String>), String> {
    let user =
        auth_repo::load_user_by_id(conn, user_id)?.ok_or_else(|| "user_not_found".to_string())?;
    let company_id = user
        .company_id
        .ok_or_else(|| "company_not_found".to_string())?;
    let role = repo::load_company_role_for_user(conn, user_id)?.map(|(_, role_key)| role_key);
    Ok((company_id, role))
}

fn can_manage_members(role: Option<&str>) -> bool {
    matches!(role, Some("owner") | Some("ceo") | Some("manager"))
}

fn sync_local_company_context(
    conn: &Connection,
    user_id: i64,
    company_id: i64,
) -> Result<(), String> {
    let now = now_rfc3339();
    let company = repo::load_company_overview(conn, company_id)?
        .ok_or_else(|| "company_not_found".to_string())?;
    let role = repo::load_company_role_for_user(conn, user_id)?
        .map(|(_, role_key)| role_key)
        .unwrap_or_else(|| "owner".to_string());
    repo::upsert_vtc_company(
        conn,
        company.id,
        &company.name,
        &company.created_at,
        &company.updated_at,
        "local_only",
        None,
    )?;
    repo::upsert_vtc_company_member(conn, company.id, user_id, &role)?;
    repo::set_local_context(conn, Some(user_id), Some(company.id), &now)?;
    Ok(())
}

fn ensure_seed_company_for_user(conn: &Connection, user_id: i64) -> Result<i64, String> {
    if let Some(company_id) =
        auth_repo::load_user_by_id(conn, user_id)?.and_then(|user| user.company_id)
    {
        if repo::load_company_overview(conn, company_id)?.is_some() {
            return Ok(company_id);
        }
    }

    let input = CreateCompanyInput {
        name: "My Company".to_string(),
        location: "Berlin".to_string(),
        language: "en".to_string(),
        game: "ETS2".to_string(),
        description: Some("Local offline-first VTC company".to_string()),
        logo_path: None,
        header_path: None,
        slogan: Some("Local operations".to_string()),
        accent_color: Some("#2F6B5F".to_string()),
        public_visibility: Some(false),
    };
    let now = now_rfc3339();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let company_id = repo::create_company(&tx, user_id, &input, &now)?;
    repo::assign_member_role(&tx, company_id, user_id, "owner", None, &now)?;
    repo::set_user_company(&tx, user_id, company_id)?;
    repo::ensure_company_settings_row(&tx, company_id, &input.language, &input.game, &now)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(company_id)
}

pub fn ensure_local_company_bootstrap(conn: &Connection, auth: &AuthState) -> Result<(), String> {
    let now = now_rfc3339();
    repo::ensure_local_context_row(conn, &now)?;

    let admin = auth_repo::find_user_by_email(conn, "admin@admin.de")?
        .ok_or_else(|| "user_not_found".to_string())?;
    let local_context = repo::load_local_context(conn)?;
    let active_user_id = local_context.active_user_id.unwrap_or(admin.id);

    if repo::count_vtc_companies(conn)? == 0 {
        let company_id = ensure_seed_company_for_user(conn, admin.id)?;
        sync_local_company_context(conn, admin.id, company_id)?;
    }

    let active_company_id = match local_context.active_company_id {
        Some(company_id) if repo::load_company_overview(conn, company_id)?.is_some() => company_id,
        _ => {
            let company_id = auth_repo::load_user_by_id(conn, active_user_id)?
                .and_then(|user| user.company_id)
                .or_else(|| {
                    auth_repo::load_user_by_id(conn, admin.id)
                        .ok()
                        .flatten()
                        .and_then(|user| user.company_id)
                })
                .ok_or_else(|| "company_not_found".to_string())?;
            sync_local_company_context(conn, active_user_id, company_id)?;
            company_id
        }
    };

    if let Some(company) = repo::load_company_overview(conn, active_company_id)? {
        repo::upsert_vtc_company(
            conn,
            company.id,
            &company.name,
            &company.created_at,
            &company.updated_at,
            "local_only",
            None,
        )?;
    }

    let role = repo::load_company_role_for_user(conn, active_user_id)?
        .map(|(_, role_key)| role_key)
        .unwrap_or_else(|| "owner".to_string());
    repo::upsert_vtc_company_member(conn, active_company_id, active_user_id, &role)?;
    repo::set_local_context(conn, Some(active_user_id), Some(active_company_id), &now)?;

    let has_session = auth
        .session
        .lock()
        .map_err(|_| "not_allowed".to_string())?
        .is_some();
    if !has_session {
        auth_service::set_local_session(auth, active_user_id)?;
    }

    Ok(())
}

pub fn get_current_user_profile(
    conn: &Connection,
    auth: &AuthState,
) -> Result<UserProfile, String> {
    let user_id = require_user_id(auth)?;
    let now = now_rfc3339();
    repo::ensure_user_settings_row(conn, user_id, &now)?;
    let mut profile =
        repo::load_user_profile(conn, user_id)?.ok_or_else(|| "user_not_found".to_string())?;
    profile.username_next_change_at =
        compute_next_username_change(profile.username_last_changed_at.as_deref());
    Ok(profile)
}

pub fn update_user_language(
    conn: &Connection,
    auth: &AuthState,
    language: String,
) -> Result<UserSettings, String> {
    let user_id = require_user_id(auth)?;
    let now = now_rfc3339();
    repo::ensure_user_settings_row(conn, user_id, &now)?;

    let input = UpdateUserSettingsInput {
        language: Some(normalize_required(&language)),
        preferred_game: None,
        profile_visibility: None,
        theme_preference: None,
        notifications_enabled: None,
    };

    repo::update_user_settings(conn, user_id, &input, &now)?;
    repo::load_user_settings(conn, user_id)
}

pub fn check_username_availability(
    conn: &Connection,
    username: String,
) -> Result<UsernameAvailability, String> {
    let normalized = match validate_username_format(&username) {
        Ok(value) => value,
        Err(_) => {
            return Ok(UsernameAvailability {
                available: false,
                reason: Some("invalid_username".to_string()),
            });
        }
    };

    let exists = repo::find_user_by_username_case_insensitive(conn, &normalized)?;
    Ok(UsernameAvailability {
        available: exists.is_none(),
        reason: if exists.is_none() {
            None
        } else {
            Some("username_already_taken".to_string())
        },
    })
}

pub fn update_username(
    conn: &mut Connection,
    auth: &AuthState,
    username: String,
) -> Result<UserProfile, String> {
    let user_id = require_user_id(auth)?;
    let now = now_rfc3339();
    let normalized = validate_username_format(&username)?;

    repo::ensure_user_settings_row(conn, user_id, &now)?;

    let user =
        auth_repo::load_user_by_id(conn, user_id)?.ok_or_else(|| "user_not_found".to_string())?;
    if user.username.eq_ignore_ascii_case(&normalized) {
        return get_current_user_profile(conn, auth);
    }

    let settings = repo::load_user_settings(conn, user_id)?;
    if let Some(next_change_at) =
        compute_next_username_change(settings.username_last_changed_at.as_deref())
    {
        let next_change = DateTime::parse_from_rfc3339(&next_change_at)
            .map_err(|_| "username_change_cooldown_active".to_string())?
            .with_timezone(&Utc);
        if Utc::now() < next_change {
            return Err("username_change_cooldown_active".to_string());
        }
    }

    if let Some((existing_user_id, _)) =
        repo::find_user_by_username_case_insensitive(conn, &normalized)?
    {
        if existing_user_id != user_id {
            return Err("username_already_taken".to_string());
        }
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    repo::update_username(&tx, user_id, &normalized, &now)?;
    repo::set_username_last_changed_at(&tx, user_id, &now)?;
    tx.commit().map_err(|e| e.to_string())?;

    get_current_user_profile(conn, auth)
}

pub fn update_user_profile_meta(
    conn: &Connection,
    auth: &AuthState,
    input: UpdateUserProfileMetaInput,
) -> Result<UserSettings, String> {
    let user_id = require_user_id(auth)?;
    let now = now_rfc3339();
    repo::ensure_user_settings_row(conn, user_id, &now)?;

    repo::update_user_profile_meta(
        conn,
        user_id,
        normalize_optional(input.avatar_path),
        normalize_optional(input.bio),
        input.profile_visibility,
        &now,
    )?;

    repo::load_user_settings(conn, user_id)
}

pub fn create_company(
    conn: &mut Connection,
    auth: &AuthState,
    mut input: CreateCompanyInput,
) -> Result<CompanyOverview, String> {
    let user_id = require_user_id(auth)?;
    input.name = normalize_required(&input.name);
    input.location = normalize_required(&input.location);
    input.language = normalize_required(&input.language);
    input.game = validate_game(&input.game)?;
    input.description = normalize_optional(input.description);
    input.logo_path = normalize_optional(input.logo_path);
    input.header_path = normalize_optional(input.header_path);
    input.slogan = normalize_optional(input.slogan);
    input.accent_color = normalize_optional(input.accent_color);

    if input.name.is_empty() {
        return Err("company_name_required".to_string());
    }
    if input.location.is_empty() {
        return Err("company_location_required".to_string());
    }
    if input.language.is_empty() {
        return Err("company_language_required".to_string());
    }

    let user =
        auth_repo::load_user_by_id(conn, user_id)?.ok_or_else(|| "user_not_found".to_string())?;
    if let Some(existing_company_id) = user.company_id {
        if repo::load_company_overview(conn, existing_company_id)?.is_some() {
            return Err("user_already_in_company".to_string());
        }
        auth_repo::clear_user_company(conn, user_id)?;
    }

    if repo::find_company_id_by_name_case_insensitive(conn, &input.name)?.is_some() {
        return Err("company_name_already_taken".to_string());
    }

    let now = now_rfc3339();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let company_id = repo::create_company(&tx, user_id, &input, &now)?;
    repo::assign_member_role(&tx, company_id, user_id, "owner", None, &now)?;
    repo::set_user_company(&tx, user_id, company_id)?;
    repo::ensure_company_settings_row(&tx, company_id, &input.language, &input.game, &now)?;
    tx.commit().map_err(|e| e.to_string())?;
    sync_local_company_context(conn, user_id, company_id)?;

    repo::load_company_overview(conn, company_id)?.ok_or_else(|| "company_not_found".to_string())
}

pub fn get_company_overview(
    conn: &Connection,
    auth: &AuthState,
) -> Result<CompanyOverview, String> {
    let user_id = require_user_id(auth)?;
    let (company_id, _) = enforce_company_access(conn, user_id)?;

    let company = repo::load_company_overview(conn, company_id)?
        .ok_or_else(|| "company_not_found".to_string())?;
    let now = now_rfc3339();
    repo::ensure_company_settings_row(
        conn,
        company_id,
        company.language.as_deref().unwrap_or("en"),
        company.game.as_deref().unwrap_or("ETS2"),
        &now,
    )?;
    sync_local_company_context(conn, user_id, company_id)?;
    Ok(company)
}

pub fn update_company_profile(
    conn: &Connection,
    auth: &AuthState,
    mut input: UpdateCompanyProfileInput,
) -> Result<CompanyOverview, String> {
    let user_id = require_user_id(auth)?;
    let (company_id, role) = enforce_company_access(conn, user_id)?;
    if !can_manage_members(role.as_deref()) {
        return Err("not_allowed".to_string());
    }

    input.name = normalize_optional(input.name);
    input.location = normalize_optional(input.location);
    input.language = normalize_optional(input.language);
    input.description = normalize_optional(input.description);
    input.logo_path = normalize_optional(input.logo_path);
    input.header_path = normalize_optional(input.header_path);
    input.slogan = normalize_optional(input.slogan);
    input.accent_color = normalize_optional(input.accent_color);
    if let Some(game) = input.game.as_ref() {
        input.game = Some(validate_game(game)?);
    }

    if let Some(name) = input.name.as_ref() {
        if let Some(existing_company_id) =
            repo::find_company_id_by_name_case_insensitive(conn, name)?
        {
            if existing_company_id != company_id {
                return Err("company_name_already_taken".to_string());
            }
        }
    }

    let now = now_rfc3339();
    repo::update_company_profile(conn, company_id, &input, &now)?;
    sync_local_company_context(conn, user_id, company_id)?;

    repo::load_company_overview(conn, company_id)?.ok_or_else(|| "company_not_found".to_string())
}

pub fn get_company_members(
    conn: &Connection,
    auth: &AuthState,
) -> Result<Vec<CompanyMember>, String> {
    let user_id = require_user_id(auth)?;
    let (company_id, _) = enforce_company_access(conn, user_id)?;
    let members = repo::load_company_members(conn, company_id)?;
    for member in &members {
        repo::upsert_vtc_company_member(conn, company_id, member.user_id, &member.role_key)?;
    }
    Ok(members)
}

pub fn get_company_settings(
    conn: &Connection,
    auth: &AuthState,
) -> Result<CompanySettings, String> {
    let user_id = require_user_id(auth)?;
    let (company_id, _) = enforce_company_access(conn, user_id)?;

    let company = repo::load_company_overview(conn, company_id)?
        .ok_or_else(|| "company_not_found".to_string())?;
    let now = now_rfc3339();
    repo::ensure_company_settings_row(
        conn,
        company_id,
        company.language.as_deref().unwrap_or("en"),
        company.game.as_deref().unwrap_or("ETS2"),
        &now,
    )?;

    repo::load_company_settings(conn, company_id)
}

pub fn update_company_settings(
    conn: &Connection,
    auth: &AuthState,
    input: UpdateCompanySettingsInput,
) -> Result<CompanySettings, String> {
    let user_id = require_user_id(auth)?;
    let (company_id, role) = enforce_company_access(conn, user_id)?;
    if !can_manage_members(role.as_deref()) {
        return Err("not_allowed".to_string());
    }

    if let Some(default_member_role) = input.default_member_role.as_ref() {
        if !repo::is_valid_role(conn, default_member_role)? {
            return Err("invalid_role".to_string());
        }
    }

    let now = now_rfc3339();
    let company = repo::load_company_overview(conn, company_id)?
        .ok_or_else(|| "company_not_found".to_string())?;
    repo::ensure_company_settings_row(
        conn,
        company_id,
        company.language.as_deref().unwrap_or("en"),
        company.game.as_deref().unwrap_or("ETS2"),
        &now,
    )?;
    repo::update_company_settings(conn, company_id, &input, &now)?;

    repo::load_company_settings(conn, company_id)
}

pub fn assign_member_role(
    conn: &Connection,
    auth: &AuthState,
    user_id: i64,
    role_key: String,
) -> Result<CompanyMember, String> {
    let actor_id = require_user_id(auth)?;
    let (company_id, actor_role) = enforce_company_access(conn, actor_id)?;
    if !can_manage_members(actor_role.as_deref()) {
        return Err("not_allowed".to_string());
    }

    if !repo::is_valid_role(conn, &role_key)? {
        return Err("invalid_role".to_string());
    }

    let target_user =
        auth_repo::load_user_by_id(conn, user_id)?.ok_or_else(|| "user_not_found".to_string())?;
    if let Some(existing_company_id) = target_user.company_id {
        if existing_company_id != company_id {
            return Err("user_already_in_company".to_string());
        }
    }

    let now = now_rfc3339();
    repo::assign_member_role(conn, company_id, user_id, &role_key, Some(actor_id), &now)?;
    repo::set_user_company(conn, user_id, company_id)?;
    repo::upsert_vtc_company_member(conn, company_id, user_id, &role_key)?;
    repo::set_local_context(conn, Some(actor_id), Some(company_id), &now)?;

    repo::load_company_member_by_user(conn, company_id, user_id)?
        .ok_or_else(|| "member_not_found".to_string())
}

pub fn change_member_role(
    conn: &Connection,
    auth: &AuthState,
    member_id: i64,
    role_key: String,
) -> Result<CompanyMember, String> {
    let actor_id = require_user_id(auth)?;
    let (company_id, actor_role) = enforce_company_access(conn, actor_id)?;
    if !can_manage_members(actor_role.as_deref()) {
        return Err("not_allowed".to_string());
    }

    if !repo::is_valid_role(conn, &role_key)? {
        return Err("invalid_role".to_string());
    }

    let current_member = repo::load_company_member_by_id(conn, company_id, member_id)?
        .ok_or_else(|| "member_not_found".to_string())?;

    if current_member.role_key == "owner" && role_key != "owner" {
        return Err("not_allowed".to_string());
    }

    let now = now_rfc3339();
    repo::change_member_role(conn, company_id, member_id, &role_key, &now)?;
    if let Some(member) = repo::load_company_member_by_id(conn, company_id, member_id)? {
        repo::upsert_vtc_company_member(conn, company_id, member.user_id, &role_key)?;
    }

    repo::load_company_member_by_id(conn, company_id, member_id)?
        .ok_or_else(|| "member_not_found".to_string())
}

pub fn get_available_roles(conn: &Connection) -> Result<Vec<CompanyRoleOption>, String> {
    repo::list_roles(conn)
}

pub fn get_user_settings(conn: &Connection, auth: &AuthState) -> Result<UserSettings, String> {
    let user_id = require_user_id(auth)?;
    let now = now_rfc3339();
    repo::ensure_user_settings_row(conn, user_id, &now)?;
    repo::load_user_settings(conn, user_id)
}

pub fn update_user_settings(
    conn: &Connection,
    auth: &AuthState,
    mut input: UpdateUserSettingsInput,
) -> Result<UserSettings, String> {
    let user_id = require_user_id(auth)?;
    let now = now_rfc3339();
    repo::ensure_user_settings_row(conn, user_id, &now)?;

    input.language = input.language.map(|value| normalize_required(&value));
    input.preferred_game = match input.preferred_game {
        Some(value) => Some(validate_game(&value)?),
        None => None,
    };
    input.profile_visibility = input
        .profile_visibility
        .map(|value| normalize_required(&value));
    input.theme_preference = input
        .theme_preference
        .map(|value| normalize_required(&value));

    repo::update_user_settings(conn, user_id, &input, &now)?;
    repo::load_user_settings(conn, user_id)
}

pub fn get_career_settings(conn: &Connection, auth: &AuthState) -> Result<CareerSettings, String> {
    let _ = require_user_id(auth)?;
    repo::load_career_settings(conn)
}

pub fn update_career_settings(
    conn: &Connection,
    auth: &AuthState,
    input: UpdateCareerSettingsInput,
) -> Result<CareerSettings, String> {
    let _ = require_user_id(auth)?;
    let now = now_rfc3339();
    repo::update_career_settings(conn, &input, &now)?;
    repo::load_career_settings(conn)
}
