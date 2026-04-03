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
        WHERE token = ?1 AND revoked_at IS NULL
        "#,
        params![token],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn touch_session(conn: &Connection, session_id: i64, last_used_at: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET last_used_at = ?1 WHERE id = ?2 AND revoked_at IS NULL",
        params![last_used_at, session_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn revoke_session(conn: &Connection, session_id: i64, revoked_at: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET revoked_at = ?1 WHERE id = ?2",
        params![revoked_at, session_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn revoke_session_by_token(conn: &Connection, token: &str, revoked_at: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET revoked_at = ?1 WHERE token = ?2",
        params![revoked_at, token],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_user_last_login_at(
    conn: &Connection,
    user_id: i64,
    last_login_at: &str,
    updated_at: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE users SET last_login_at = ?1, updated_at = ?2 WHERE id = ?3",
        params![last_login_at, updated_at, user_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_users_basic(
    conn: &Connection,
    limit: i64,
) -> Result<Vec<(i64, String, String, String, Option<i64>, String, Option<String>)>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                username,
                email,
                role,
                company_id,
                created_at,
                last_login_at
            FROM users
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut users = Vec::new();
    for row in rows {
        users.push(row.map_err(|e| e.to_string())?);
    }
    Ok(users)
}

pub fn user_has_active_session(conn: &Connection, user_id: i64, now_rfc3339: &str) -> Result<bool, String> {
    conn.query_row(
        r#"
        SELECT 1
        FROM sessions
        WHERE user_id = ?1
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > ?2)
        LIMIT 1
        "#,
        params![user_id, now_rfc3339],
        |_row| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|e| e.to_string())
}

pub fn list_sessions_by_user_id(
    conn: &Connection,
    user_id: i64,
    limit: i64,
) -> Result<Vec<(i64, String, Option<String>, Option<String>)>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, created_at, expires_at, last_used_at
            FROM sessions
            WHERE user_id = ?1
            ORDER BY COALESCE(last_used_at, created_at) DESC
            LIMIT ?2
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![user_id, limit], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row.map_err(|e| e.to_string())?);
    }
    Ok(sessions)
}

pub fn count_unused_recovery_codes(conn: &Connection, user_id: i64) -> Result<u32, String> {
    conn.query_row(
        "SELECT COUNT(1) FROM recovery_codes WHERE user_id = ?1 AND used_at IS NULL",
        params![user_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value.max(0) as u32)
    .map_err(|e| e.to_string())
}

pub fn delete_recovery_codes_for_user(conn: &Connection, user_id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM recovery_codes WHERE user_id = ?1", params![user_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_recovery_code(
    conn: &Connection,
    user_id: i64,
    code_hash: &str,
    created_at: &str,
) -> Result<i64, String> {
    conn.execute(
        r#"
        INSERT INTO recovery_codes (user_id, code_hash, created_at, used_at)
        VALUES (?1, ?2, ?3, NULL)
        "#,
        params![user_id, code_hash, created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn list_unused_recovery_codes(
    conn: &Connection,
    user_id: i64,
) -> Result<Vec<(i64, String)>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, code_hash
            FROM recovery_codes
            WHERE user_id = ?1 AND used_at IS NULL
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![user_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?;

    let mut codes = Vec::new();
    for row in rows {
        codes.push(row.map_err(|e| e.to_string())?);
    }
    Ok(codes)
}

pub fn mark_recovery_code_used(
    conn: &Connection,
    code_id: i64,
    used_at: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE recovery_codes SET used_at = ?1 WHERE id = ?2",
        params![used_at, code_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_login_event(
    conn: &Connection,
    user_id: i64,
    at_utc: &str,
    year_month: &str,
) -> Result<i64, String> {
    conn.execute(
        r#"
        INSERT INTO login_events (user_id, at_utc, year_month)
        VALUES (?1, ?2, ?3)
        "#,
        params![user_id, at_utc, year_month],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn has_install_activity_for_month(conn: &Connection, year_month: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM login_events WHERE year_month = ?1 LIMIT 1",
        params![year_month],
        |_row| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|e| e.to_string())
}

pub fn has_user_activity_for_month(
    conn: &Connection,
    user_id: i64,
    year_month: &str,
) -> Result<bool, String> {
    conn.query_row(
        "SELECT 1 FROM login_events WHERE user_id = ?1 AND year_month = ?2 LIMIT 1",
        params![user_id, year_month],
        |_row| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|e| e.to_string())
}

pub fn count_active_accounts_for_month(conn: &Connection, year_month: &str) -> Result<u32, String> {
    conn.query_row(
        "SELECT COUNT(DISTINCT user_id) FROM login_events WHERE year_month = ?1 AND user_id IS NOT NULL",
        params![year_month],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value.max(0) as u32)
    .map_err(|e| e.to_string())
}
