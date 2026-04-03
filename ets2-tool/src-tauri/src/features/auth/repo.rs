use rusqlite::{params, Connection, OptionalExtension};

use crate::features::auth::models::{NewSession, NewUser, UserRecord};

pub fn find_user_by_email(conn: &Connection, email: &str) -> Result<Option<UserRecord>, String> {
    conn.query_row(
        r#"
        SELECT
            id,
            username,
            email,
            password_hash,
            role,
            company_id,
            created_at,
            updated_at,
            consent_at,
            is_active,
            is_seed
        FROM users
        WHERE email = ?1
        "#,
        params![email],
        |row| {
            Ok(UserRecord {
                id: row.get(0)?,
                username: row.get(1)?,
                email: row.get(2)?,
                password_hash: row.get(3)?,
                role: row.get(4)?,
                company_id: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                consent_at: row.get(8)?,
                is_active: row.get::<_, i64>(9)? != 0,
                is_seed: row.get::<_, i64>(10)? != 0,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn find_user_by_username(
    conn: &Connection,
    username: &str,
) -> Result<Option<UserRecord>, String> {
    conn.query_row(
        r#"
        SELECT
            id,
            username,
            email,
            password_hash,
            role,
            company_id,
            created_at,
            updated_at,
            consent_at,
            is_active,
            is_seed
        FROM users
        WHERE username = ?1
        "#,
        params![username],
        |row| {
            Ok(UserRecord {
                id: row.get(0)?,
                username: row.get(1)?,
                email: row.get(2)?,
                password_hash: row.get(3)?,
                role: row.get(4)?,
                company_id: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                consent_at: row.get(8)?,
                is_active: row.get::<_, i64>(9)? != 0,
                is_seed: row.get::<_, i64>(10)? != 0,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn insert_user(conn: &Connection, user: &NewUser) -> Result<i64, String> {
    conn.execute(
        r#"
        INSERT INTO users (
            username,
            email,
            password_hash,
            role,
            company_id,
            created_at,
            updated_at,
            consent_at,
            is_active,
            is_seed
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
        params![
            user.username,
            user.email,
            user.password_hash,
            user.role,
            user.company_id,
            user.created_at,
            user.updated_at,
            user.consent_at,
            if user.is_active { 1 } else { 0 },
            if user.is_seed { 1 } else { 0 }
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

pub fn update_user_company(conn: &Connection, user_id: i64, company_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE users SET company_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![company_id, chrono::Utc::now().to_rfc3339(), user_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn clear_user_company(conn: &Connection, user_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE users SET company_id = NULL, updated_at = ?1 WHERE id = ?2",
        params![chrono::Utc::now().to_rfc3339(), user_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_user_password_hash(
    conn: &Connection,
    user_id: i64,
    password_hash: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3",
        params![password_hash, chrono::Utc::now().to_rfc3339(), user_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_user_by_id(conn: &Connection, user_id: i64) -> Result<Option<UserRecord>, String> {
    conn.query_row(
        r#"
        SELECT
            id,
            username,
            email,
            password_hash,
            role,
            company_id,
            created_at,
            updated_at,
            consent_at,
            is_active,
            is_seed
        FROM users
        WHERE id = ?1
        "#,
        params![user_id],
        |row| {
            Ok(UserRecord {
                id: row.get(0)?,
                username: row.get(1)?,
                email: row.get(2)?,
                password_hash: row.get(3)?,
                role: row.get(4)?,
                company_id: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                consent_at: row.get(8)?,
                is_active: row.get::<_, i64>(9)? != 0,
                is_seed: row.get::<_, i64>(10)? != 0,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn insert_session(conn: &Connection, session: &NewSession) -> Result<i64, String> {
    conn.execute(
        r#"
        INSERT INTO sessions (
            user_id,
            token,
            created_at,
            expires_at,
            last_used_at
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            session.user_id,
            session.token,
            session.created_at,
            session.expires_at,
            session.last_used_at
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn find_session_by_token(
    conn: &Connection,
    token: &str,
) -> Result<Option<(i64, i64, String, String)>, String> {
    conn.query_row(
        r#"
        SELECT id, user_id, expires_at, last_used_at
        FROM sessions
        WHERE token = ?1
        "#,
        params![token],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn touch_session(conn: &Connection, session_id: i64, last_used_at: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET last_used_at = ?1 WHERE id = ?2",
        params![last_used_at, session_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
