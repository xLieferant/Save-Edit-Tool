use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

pub fn default_db_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("SimNexusHub")
        .join("logbook.sqlite")
}

pub fn auth_session_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("SimNexusHub")
        .join("auth_session.json")
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL,
            email TEXT NOT NULL,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'user',
            company_id INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            consent_at TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 1,
            is_seed INTEGER NOT NULL DEFAULT 0
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username ON users(username);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email ON users(email);

        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            token TEXT,
            created_at TEXT NOT NULL,
            expires_at TEXT,
            last_used_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token);
        "#,
    )
    .map_err(|e| e.to_string())?;

    ensure_user_columns(conn)?;
    Ok(())
}

fn ensure_user_columns(conn: &Connection) -> Result<(), String> {
    let existing = existing_columns(conn, "users")?;
    let required = [
        ("role", "TEXT NOT NULL DEFAULT 'user'"),
        ("company_id", "INTEGER"),
        ("consent_at", "TEXT NOT NULL DEFAULT ''"),
        ("is_active", "INTEGER NOT NULL DEFAULT 1"),
        ("is_seed", "INTEGER NOT NULL DEFAULT 0"),
    ];

    for (column, definition) in required {
        if !existing.contains(column) {
            conn.execute(&format!("ALTER TABLE users ADD COLUMN {column} {definition}"), [])
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn existing_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;

    let mut columns = HashSet::new();
    for row in rows {
        columns.insert(row.map_err(|e| e.to_string())?);
    }

    Ok(columns)
}

pub fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

