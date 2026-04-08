use std::path::{Path, PathBuf};

use crate::shared::sqlite_schema::ensure_columns;
use chrono::Utc;
use rusqlite::{Connection as RusqliteConnection, OptionalExtension};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const MIGRATION_FILES: [&str; 10] = [
    "2026-04-06_create_ets_profiles.sql",
    "2026-04-06_create_ets_saves.sql",
    "2026-04-06_create_ets_job_links.sql",
    "2026-04-06_create_ets_job_link_audit.sql",
    "2026-04-06_create_vtc_job_ledger.sql",
    "2026-04-06_create_ets2_datasets.sql",
    "2026-04-06_create_ets_save_snapshot.sql",
    "2026-04-06_add_resolved_tokens_to_ets_job_links.sql",
    "2026-04-06_add_cargo_resolution_to_ets_job_links.sql",
    "2026-04-07_add_vtc_local_persistence.sql",
];

pub fn app_db_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .join("data")
        .join("app.sqlite")
}

pub async fn init_sqlite() -> Result<SqlitePool, String> {
    let db_path = app_db_path();
    validate_sqlite_extension(&db_path)?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    run_runtime_migrations(&db_path)?;

    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .map_err(|error| error.to_string())?;

    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("PRAGMA synchronous = NORMAL;")
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("PRAGMA temp_store = MEMORY;")
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;

    crate::dev_log!("[db] Using SQLite DB: {}", db_path.display());
    Ok(pool)
}

pub fn validate_sqlite_extension(path: &Path) -> Result<(), String> {
    let ends_with_sqlite = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("sqlite"))
        .unwrap_or(false);
    if !ends_with_sqlite {
        return Err(format!(
            "sqlite path must end with .sqlite, got {}",
            path.display()
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqliteInfoDto {
    pub path: String,
    pub tables: Vec<String>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqliteTableCountsDto {
    pub path: String,
    pub active_save_session_id: Option<String>,
    pub ets2_companies: i64,
    pub ets_save_snapshot: i64,
    pub ets_save_depots: i64,
    pub ets_save_visited_cities: i64,
    pub ets_save_transport_cargo: i64,
    pub ets_save_snapshot_meta: i64,
    pub ets_job_links: i64,
    pub dispatcher_jobs: i64,
    pub vtc_companies: i64,
    pub vtc_company_members: i64,
    pub vtc_local_context: i64,
}

pub async fn get_sqlite_info(pool: &SqlitePool) -> Result<SqliteInfoDto, String> {
    let db_path = resolve_sqlite_main_path(pool).await?;
    let rows = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| error.to_string())?;
    let tables = rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .collect::<Vec<_>>();
    let size_bytes = std::fs::metadata(&db_path)
        .map(|meta| meta.len())
        .unwrap_or(0);

    Ok(SqliteInfoDto {
        path: db_path,
        tables,
        size_bytes,
    })
}

pub async fn get_sqlite_table_counts(
    pool: &SqlitePool,
    active_save_session_id: Option<String>,
) -> Result<SqliteTableCountsDto, String> {
    let path = resolve_sqlite_main_path(pool).await?;
    Ok(SqliteTableCountsDto {
        path,
        active_save_session_id,
        ets2_companies: count_rows(pool, "ets2_companies").await?,
        ets_save_snapshot: count_rows(pool, "ets_save_snapshot").await?,
        ets_save_depots: count_rows(pool, "ets_save_depots").await?,
        ets_save_visited_cities: count_rows(pool, "ets_save_visited_cities").await?,
        ets_save_transport_cargo: count_rows(pool, "ets_save_transport_cargo").await?,
        ets_save_snapshot_meta: count_rows(pool, "ets_save_snapshot_meta").await?,
        ets_job_links: count_rows(pool, "ets_job_links").await?,
        dispatcher_jobs: count_rows(pool, "dispatcher_jobs").await?,
        vtc_companies: count_rows(pool, "vtc_companies").await?,
        vtc_company_members: count_rows(pool, "vtc_company_members").await?,
        vtc_local_context: count_rows(pool, "vtc_local_context").await?,
    })
}

async fn resolve_sqlite_main_path(pool: &SqlitePool) -> Result<String, String> {
    let rows = sqlx::query("PRAGMA database_list")
        .fetch_all(pool)
        .await
        .map_err(|error| error.to_string())?;
    for row in rows {
        let name: String = row
            .try_get("name")
            .map_err(|error| format!("read db_list name failed: {}", error))?;
        if name == "main" {
            let file: String = row
                .try_get("file")
                .map_err(|error| format!("read db_list file failed: {}", error))?;
            return Ok(file);
        }
    }
    Err("sqlite main database path not found".to_string())
}

async fn count_rows(pool: &SqlitePool, table: &str) -> Result<i64, String> {
    let table_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())?;

    if table_exists == 0 {
        return Ok(0);
    }

    let sql = format!("SELECT COUNT(*) AS c FROM {}", table);
    sqlx::query_scalar::<_, i64>(&sql)
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())
}

fn migration_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("db")
        .join("migrations")
}

fn run_runtime_migrations(db_path: &Path) -> Result<(), String> {
    let mut connection = RusqliteConnection::open(db_path).map_err(|error| error.to_string())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS ets_feature_migrations (
            filename TEXT PRIMARY KEY,
            applied_at_utc TEXT NOT NULL
        );
        "#,
        )
        .map_err(|error| error.to_string())?;

    let tx = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let migration_dir = migration_directory();

    for filename in MIGRATION_FILES {
        let already_applied: Option<String> = tx
            .query_row(
                "SELECT filename FROM ets_feature_migrations WHERE filename = ?1",
                [filename],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if already_applied.is_some() {
            continue;
        }

        let migration_path = migration_dir.join(filename);
        let sql = std::fs::read_to_string(&migration_path).map_err(|error| {
            format!(
                "read migration {} failed: {}",
                migration_path.display(),
                error
            )
        })?;
        tx.execute_batch(&sql)
            .map_err(|error| format!("apply migration {} failed: {}", filename, error))?;
        tx.execute(
            "INSERT INTO ets_feature_migrations (filename, applied_at_utc) VALUES (?1, ?2)",
            rusqlite::params![filename, Utc::now().to_rfc3339()],
        )
        .map_err(|error| format!("record migration {} failed: {}", filename, error))?;
    }

    tx.commit().map_err(|error| error.to_string())?;
    ensure_runtime_columns(&connection)?;
    Ok(())
}

fn ensure_runtime_columns(conn: &RusqliteConnection) -> Result<(), String> {
    ensure_columns(
        conn,
        "ets_save_depots",
        &[
            ("discovered", "INTEGER NOT NULL DEFAULT 1"),
            ("job_offer_count", "INTEGER NOT NULL DEFAULT 0"),
        ],
    )?;
    conn.execute(
        "UPDATE ets_save_depots SET job_offer_count = 0 WHERE job_offer_count IS NULL",
        [],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}
