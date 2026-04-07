use rusqlite::{Connection, params};

use crate::features::auth;
use crate::features::companies;
use crate::shared::sqlite_schema::ensure_columns;

pub const AVAILABLE_ROLE_KEYS: [&str; 8] = [
    "owner",
    "ceo",
    "manager",
    "dispatcher",
    "driver",
    "trainee",
    "recruiter",
    "mechanic",
];

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    auth::db::ensure_tables(conn)?;
    companies::db::ensure_tables(conn)?;

    ensure_company_columns(conn)?;
    ensure_company_member_columns(conn)?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS company_roles (
            role_key TEXT PRIMARY KEY,
            role_label TEXT NOT NULL,
            sort_order INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS user_settings (
            user_id INTEGER PRIMARY KEY,
            language TEXT NOT NULL DEFAULT 'en',
            preferred_game TEXT NOT NULL DEFAULT 'ETS2',
            profile_visibility TEXT NOT NULL DEFAULT 'private',
            username_last_changed_at TEXT,
            theme_preference TEXT,
            notifications_enabled INTEGER NOT NULL DEFAULT 1,
            avatar_path TEXT,
            bio TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS company_settings (
            company_id INTEGER PRIMARY KEY,
            company_language TEXT NOT NULL DEFAULT 'en',
            company_game TEXT NOT NULL DEFAULT 'ETS2',
            allow_public_join_requests INTEGER NOT NULL DEFAULT 0,
            show_company_publicly INTEGER NOT NULL DEFAULT 1,
            default_member_role TEXT NOT NULL DEFAULT 'driver',
            dispatcher_can_manage_jobs INTEGER NOT NULL DEFAULT 1,
            trainee_visible_in_roster INTEGER NOT NULL DEFAULT 1,
            allow_member_custom_profiles INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS career_settings (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            telemetry_enabled INTEGER NOT NULL DEFAULT 1,
            local_stats_tracking_enabled INTEGER NOT NULL DEFAULT 1,
            auto_job_logging_enabled INTEGER NOT NULL DEFAULT 1,
            auto_finance_tracking_enabled INTEGER NOT NULL DEFAULT 1,
            use_metric_units INTEGER NOT NULL DEFAULT 1,
            use_24h_time INTEGER NOT NULL DEFAULT 1,
            autosave_career_data INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS vtc_companies (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            sync_state TEXT NOT NULL DEFAULT 'local_only',
            remote_id TEXT
        );

        CREATE TABLE IF NOT EXISTS vtc_company_members (
            company_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            role TEXT NOT NULL,
            PRIMARY KEY (company_id, user_id)
        );

        CREATE TABLE IF NOT EXISTS vtc_local_context (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            active_user_id INTEGER,
            active_company_id INTEGER,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    seed_roles(conn)?;
    ensure_global_career_settings(conn)?;
    ensure_local_context(conn)?;
    Ok(())
}

fn ensure_company_columns(conn: &Connection) -> Result<(), String> {
    let required = [
        ("slogan", "TEXT"),
        ("accent_color", "TEXT"),
        ("public_visibility", "INTEGER NOT NULL DEFAULT 1"),
    ];
    ensure_columns(conn, "companies", &required)?;
    Ok(())
}

fn ensure_company_member_columns(conn: &Connection) -> Result<(), String> {
    let required = [
        ("promoted_at", "TEXT"),
        ("invited_by", "INTEGER"),
        ("notes", "TEXT"),
        ("updated_at", "TEXT"),
    ];
    ensure_columns(conn, "company_members", &required)?;
    Ok(())
}

fn seed_roles(conn: &Connection) -> Result<(), String> {
    let rows: [(&str, &str, i64); 8] = [
        ("owner", "Owner", 1),
        ("ceo", "CEO", 2),
        ("manager", "Manager", 3),
        ("dispatcher", "Dispatcher", 4),
        ("driver", "Driver", 5),
        ("trainee", "Trainee", 6),
        ("recruiter", "Recruiter", 7),
        ("mechanic", "Mechanic", 8),
    ];

    for (role_key, role_label, sort_order) in rows {
        conn.execute(
            r#"
            INSERT INTO company_roles (role_key, role_label, sort_order)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(role_key) DO UPDATE SET
                role_label = excluded.role_label,
                sort_order = excluded.sort_order
            "#,
            params![role_key, role_label, sort_order],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn ensure_global_career_settings(conn: &Connection) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        r#"
        INSERT OR IGNORE INTO career_settings (
            id,
            telemetry_enabled,
            local_stats_tracking_enabled,
            auto_job_logging_enabled,
            auto_finance_tracking_enabled,
            use_metric_units,
            use_24h_time,
            autosave_career_data,
            created_at,
            updated_at
        ) VALUES (1, 1, 1, 1, 1, 1, 1, 1, ?1, ?2)
        "#,
        params![now, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_local_context(conn: &Connection) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        r#"
        INSERT OR IGNORE INTO vtc_local_context (
            id,
            active_user_id,
            active_company_id,
            updated_at
        ) VALUES (1, NULL, NULL, ?1)
        "#,
        params![now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
