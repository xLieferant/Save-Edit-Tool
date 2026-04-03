use std::fs;

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;
use regex::Regex;
use rusqlite::Connection;
use uuid::Uuid;

use crate::features::auth::db;
use crate::features::auth::models::{NewSession, NewUser, PublicUser, UserRecord};
use crate::features::auth::repo;
use crate::state::{AuthSession, AuthState};

const DEFAULT_REMEMBER_DAYS: i64 = 30;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedSession {
    token: String,
}

pub fn validate_email(email: &str) -> Result<(), String> {
    let normalized = email.trim().to_lowercase();
    if normalized.is_empty() {
        return Err("Email is required".to_string());
    }

    let re = Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").map_err(|e| e.to_string())?;
    if !re.is_match(&normalized) {
        return Err("Email is invalid".to_string());
    }

    Ok(())
}

pub fn validate_username(username: &str) -> Result<(), String> {
    let normalized = username.trim();
    if normalized.is_empty() {
        return Err("Username is required".to_string());
    }
    if normalized.len() < 3 {
        return Err("Username is too short".to_string());
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters".to_string());
    }
    Ok(())
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, String> {
    let parsed = PasswordHash::new(password_hash).map_err(|e| e.to_string())?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn seed_default_admin(conn: &Connection) -> Result<(), String> {
    let admin_email = "admin@admin.de";
    let existing = repo::find_user_by_email(conn, admin_email)?;
    if let Some(existing) = existing {
        if existing.is_seed {
            let password_hash = hash_password("admin123")?;
            repo::update_user_password_hash(conn, existing.id, &password_hash)?;
        }
        return Ok(());
    }

    let created_at = now_rfc3339();
    let password_hash = hash_password("admin123")?;
    let user = NewUser {
        username: "Admin".to_string(),
        email: admin_email.to_string(),
        password_hash,
        role: "admin".to_string(),
        company_id: None,
        created_at: created_at.clone(),
        updated_at: created_at.clone(),
        consent_at: created_at.clone(),
        is_active: true,
        is_seed: true,
    };

    repo::insert_user(conn, &user)?;
    Ok(())
}

pub fn register_local(
    conn: &Connection,
    auth: &AuthState,
    username: String,
    email: String,
    password: String,
    password_confirm: String,
    consent_privacy: bool,
    consent_terms: bool,
    remember_me: bool,
) -> Result<(PublicUser, bool), String> {
    validate_username(&username)?;
    validate_email(&email)?;
    validate_password(&password)?;
    if password != password_confirm {
        return Err("Password confirmation does not match".to_string());
    }
    if !(consent_privacy && consent_terms) {
        return Err("Consent is required".to_string());
    }

    let normalized_email = email.trim().to_lowercase();
    let normalized_username = username.trim().to_string();

    if repo::find_user_by_email(conn, &normalized_email)?.is_some() {
        return Err("Email is already registered".to_string());
    }
    if repo::find_user_by_username(conn, &normalized_username)?.is_some() {
        return Err("Username is already taken".to_string());
    }

    let now = now_rfc3339();
    let new_user = NewUser {
        username: normalized_username,
        email: normalized_email,
        password_hash: hash_password(&password)?,
        role: "user".to_string(),
        company_id: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        consent_at: now.clone(),
        is_active: true,
        is_seed: false,
    };

    let user_id = repo::insert_user(conn, &new_user)?;
    let record = repo::load_user_by_id(conn, user_id)?
        .ok_or_else(|| "Failed to load created user".to_string())?;

    set_logged_in_user(conn, auth, &record, remember_me)?;
    Ok((PublicUser::from(record), remember_me))
}

pub fn login_local(
    conn: &Connection,
    auth: &AuthState,
    email: String,
    password: String,
    remember_me: bool,
) -> Result<(PublicUser, bool), String> {
    validate_email(&email)?;
    let normalized_email = email.trim().to_lowercase();
    let user = match repo::find_user_by_email(conn, &normalized_email)? {
        Some(user) => user,
        None => return Err("Email not found".to_string()),
    };

    if !user.is_active {
        return Err("Account is disabled".to_string());
    }

    let ok = verify_password(&password, &user.password_hash)?;
    if !ok {
        return Err("Invalid password".to_string());
    }

    set_logged_in_user(conn, auth, &user, remember_me)?;
    Ok((PublicUser::from(user), remember_me))
}

pub fn logout_local(auth: &AuthState) -> Result<(), String> {
    {
        let mut guard = auth
            .session
            .lock()
            .map_err(|_| "AuthState session lock poisoned".to_string())?;
        *guard = None;
    }

    let session_path = db::auth_session_path();
    if session_path.exists() {
        fs::remove_file(&session_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn get_current_user(conn: &Connection, auth: &AuthState) -> Result<Option<PublicUser>, String> {
    let session = {
        auth.session
            .lock()
            .map_err(|_| "AuthState session lock poisoned".to_string())?
            .clone()
    };
    let Some(session) = session else {
        return Ok(None);
    };

    let user = repo::load_user_by_id(conn, session.user_id)?;
    Ok(user.map(PublicUser::from))
}

pub fn restore_persisted_session(conn: &Connection, auth: &AuthState) -> Result<(), String> {
    let session_path = db::auth_session_path();
    if !session_path.exists() {
        return Ok(());
    }

    let payload: PersistedSession = serde_json::from_str(
        &fs::read_to_string(&session_path).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let (session_id, user_id, expires_at, _last_used_at) =
        match repo::find_session_by_token(conn, &payload.token)? {
            Some(value) => value,
            None => return Ok(()),
        };

    let expires = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|e| e.to_string())?
        .with_timezone(&chrono::Utc);
    if chrono::Utc::now() > expires {
        return Ok(());
    }

    let now = now_rfc3339();
    repo::touch_session(conn, session_id, &now)?;
    {
        let mut guard = auth
            .session
            .lock()
            .map_err(|_| "AuthState session lock poisoned".to_string())?;
        *guard = Some(AuthSession {
            user_id,
            remember_me: true,
            token: Some(payload.token),
            expires_at: Some(expires_at),
        });
    }

    Ok(())
}

fn set_logged_in_user(
    conn: &Connection,
    auth: &AuthState,
    user: &UserRecord,
    remember_me: bool,
) -> Result<(), String> {
    let mut token = None;
    let mut expires_at = None;

    if remember_me {
        token = Some(Uuid::new_v4().to_string());
        let expires = chrono::Utc::now() + chrono::Duration::days(DEFAULT_REMEMBER_DAYS);
        expires_at = Some(expires.to_rfc3339());

        let now = now_rfc3339();
        let session = NewSession {
            user_id: user.id,
            token: token.clone().ok_or_else(|| "Token missing".to_string())?,
            created_at: now.clone(),
            expires_at: expires_at.clone().ok_or_else(|| "Expires missing".to_string())?,
            last_used_at: now.clone(),
        };
        repo::insert_session(conn, &session)?;

        let session_path = db::auth_session_path();
        db::ensure_parent_dir(&session_path)?;
        let persisted = PersistedSession {
            token: token.clone().ok_or_else(|| "Token missing".to_string())?,
        };
        fs::write(&session_path, serde_json::to_string_pretty(&persisted).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    } else {
        let session_path = db::auth_session_path();
        if session_path.exists() {
            fs::remove_file(&session_path).map_err(|e| e.to_string())?;
        }
    }

    {
        let mut guard = auth
            .session
            .lock()
            .map_err(|_| "AuthState session lock poisoned".to_string())?;
        *guard = Some(AuthSession {
            user_id: user.id,
            remember_me,
            token,
            expires_at,
        });
    }

    Ok(())
}
