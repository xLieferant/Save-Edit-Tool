use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{Connection as RusqliteConnection, OptionalExtension};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, Sqlite, SqlitePool};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::events::{
    EVT_DISPATCHER_JOB_UPDATED, EVT_DISPATCHER_JOBS_UPDATED, EVT_JOB_LINK_UPDATED,
};
use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::features::ets2save::injector::{
    build_offer_patch, preview_offer_pointers, write_job_offer_patch,
};
use crate::features::ets2save::locator::resolve_last_quicksave;
use crate::features::ets2save::models::{
    EtsJobLink, EtsJobLinkStatus, EtsSaveSlot, VtcDispatcherJob,
};
use crate::features::ets2save::parser::sii_token;
use crate::features::telemetry::events::TelemetryJobEventPayload;
use crate::state::AppProfileState;

const MIGRATION_FILES: [&str; 6] = [
    "2026-04-06_create_ets_profiles.sql",
    "2026-04-06_create_ets_saves.sql",
    "2026-04-06_create_ets_job_links.sql",
    "2026-04-06_create_ets_job_link_audit.sql",
    "2026-04-06_create_vtc_job_ledger.sql",
    "2026-04-06_create_ets2_datasets.sql",
];

pub async fn create_pool(db_path: &std::path::Path) -> Result<SqlitePool, AppError> {
    run_runtime_migrations(db_path)?;

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    Ok(pool)
}

fn migration_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("db")
        .join("migrations")
}

fn run_runtime_migrations(db_path: &Path) -> Result<(), AppError> {
    let mut connection = RusqliteConnection::open(db_path).map_err(|error| {
        AppError::new(
            AppErrorCode::WriteFailed,
            format!("open migration db failed: {}", error),
        )
    })?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| {
            AppError::new(
                AppErrorCode::WriteFailed,
                format!("set migration busy timeout failed: {}", error),
            )
        })?;
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
        .map_err(|error| {
            AppError::new(
                AppErrorCode::WriteFailed,
                format!("prepare migration table failed: {}", error),
            )
        })?;

    let tx = connection.transaction().map_err(|error| {
        AppError::new(
            AppErrorCode::WriteFailed,
            format!("begin migration transaction failed: {}", error),
        )
    })?;
    let migration_dir = migration_directory();

    for filename in MIGRATION_FILES {
        let already_applied: Option<String> = tx
            .query_row(
                "SELECT filename FROM ets_feature_migrations WHERE filename = ?1",
                [filename],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                AppError::new(
                    AppErrorCode::WriteFailed,
                    format!("check migration state failed for {}: {}", filename, error),
                )
            })?;
        if already_applied.is_some() {
            continue;
        }

        let migration_path = migration_dir.join(filename);
        let sql = fs::read_to_string(&migration_path).map_err(|error| {
            AppError::new(
                AppErrorCode::WriteFailed,
                format!(
                    "read migration {} failed: {}",
                    migration_path.display(),
                    error
                ),
            )
        })?;
        tx.execute_batch(&sql).map_err(|error| {
            AppError::new(
                AppErrorCode::WriteFailed,
                format!("apply migration {} failed: {}", filename, error),
            )
        })?;
        tx.execute(
            "INSERT INTO ets_feature_migrations (filename, applied_at_utc) VALUES (?1, ?2)",
            rusqlite::params![filename, Utc::now().to_rfc3339()],
        )
        .map_err(|error| {
            AppError::new(
                AppErrorCode::WriteFailed,
                format!("record migration {} failed: {}", filename, error),
            )
        })?;
    }

    tx.commit().map_err(|error| {
        AppError::new(
            AppErrorCode::WriteFailed,
            format!("commit migration transaction failed: {}", error),
        )
    })?;
    Ok(())
}

pub async fn ets_get_last_quicksave(
    pool: &SqlitePool,
    profile_id: &str,
    state: &AppProfileState,
) -> Result<EtsSaveSlot, AppError> {
    let (_, save_slot) = resolve_last_quicksave(pool, profile_id, state).await?;
    Ok(save_slot)
}

pub async fn ets_prepare_job_link(
    app: &AppHandle,
    pool: &SqlitePool,
    vtc_job_id: &str,
    profile_id: &str,
    state: &AppProfileState,
) -> Result<EtsJobLink, AppError> {
    prepare_job_link(Some(app), pool, vtc_job_id, profile_id, state).await
}

pub async fn prepare_job_link(
    app: Option<&AppHandle>,
    pool: &SqlitePool,
    vtc_job_id: &str,
    profile_id: &str,
    state: &AppProfileState,
) -> Result<EtsJobLink, AppError> {
    let (profile, save_slot) = resolve_last_quicksave(pool, profile_id, state).await?;
    let mut connection = begin_immediate(pool).await?;
    let result = async {
        let dispatcher_job = load_vtc_dispatcher_job(&mut connection, vtc_job_id).await?;
        let src_company = sii_token(&dispatcher_job.company_id);
        let src_city = sii_token(&dispatcher_job.origin_city);
        let dst_company = sii_token(&dispatcher_job.company_id);
        let (lines, offer_pointer, offer_range) = preview_offer_pointers(
            std::path::Path::new(&save_slot.game_sii_path),
            &src_company,
            &src_city,
        )?;
        let existing_link = ensure_prepare_link_state(&mut connection, vtc_job_id).await?;
        let link_id = existing_link
            .as_ref()
            .map(|link| link.link_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let patch = build_offer_patch(&dispatcher_job, &lines, offer_range, &dst_company, &link_id);
        let dst_city = sii_token(&dispatcher_job.destination_city);
        let cargo_id = sii_token(&dispatcher_job.cargo_type);
        let now = Utc::now().to_rfc3339();
        let patch_json = serde_json::to_string(&patch)
            .map_err(|error| AppError::new(AppErrorCode::WriteFailed, error.to_string()))?;
        let previous_status = upsert_prepared_job_link(
            &mut connection,
            existing_link.as_ref(),
            PreparedJobLinkUpsert {
                link_id: &link_id,
                profile_id: &profile.profile_id,
                save_id: &save_slot.save_id,
                vtc_job_id,
                offer_pointer: &offer_pointer,
                src_company: &src_company,
                src_city: &src_city,
                dst_company: &dst_company,
                dst_city: &dst_city,
                cargo_id: &cargo_id,
                distance_km: dispatcher_job.route_distance_km,
                planned_reward: dispatcher_job.total_reward,
                patch_json: &patch_json,
                now: &now,
            },
        )
        .await?;

        insert_audit(
            &mut connection,
            &link_id,
            previous_status,
            EtsJobLinkStatus::Prepared,
            Some(&patch_json),
        )
        .await?;
        set_dispatcher_link_status(&mut connection, vtc_job_id, EtsJobLinkStatus::Prepared).await?;

        load_job_link_by_id(&mut connection, &link_id).await
    }
    .await;

    complete_transaction(&mut connection, result)
        .await
        .map(|link| {
            emit_job_link_events(app, &link);
            link
        })
}

pub async fn ets_write_job_to_quicksave(
    app: &AppHandle,
    pool: &SqlitePool,
    link_id: &str,
) -> Result<EtsJobLink, AppError> {
    let mut connection = begin_immediate(pool).await?;
    let result = async {
        let link = load_job_link_by_id(&mut connection, link_id).await?;
        let save_slot = load_save_slot(&mut connection, &link.save_id).await?;
        let pointers = write_job_offer_patch(
            std::path::Path::new(&save_slot.game_sii_path),
            &link.src_company,
            &link.src_city,
            &link.patch,
        )?;
        let now = Utc::now().to_rfc3339();

        update_status_only(
            &mut connection,
            link_id,
            EtsJobLinkStatus::RequiresLoad,
            Some(&now),
            Some(&pointers.offer_pointer),
            Some(&pointers.job_offer_data_pointer),
            None,
            None,
        )
        .await?;
        insert_audit(
            &mut connection,
            link_id,
            Some(link.status),
            EtsJobLinkStatus::Written,
            None,
        )
        .await?;
        insert_audit(
            &mut connection,
            link_id,
            Some(EtsJobLinkStatus::Written),
            EtsJobLinkStatus::RequiresLoad,
            Some(
                &serde_json::json!({
                    "backupPath": pointers.backup_path.display().to_string()
                })
                .to_string(),
            ),
        )
        .await?;
        set_dispatcher_link_status(
            &mut connection,
            &link.vtc_job_id,
            EtsJobLinkStatus::RequiresLoad,
        )
        .await?;

        load_job_link_by_id(&mut connection, link_id).await
    }
    .await;

    complete_transaction(&mut connection, result)
        .await
        .map(|link| {
            emit_job_link_events(Some(app), &link);
            link
        })
}

pub async fn ets_get_job_link_status(
    pool: &SqlitePool,
    vtc_job_id: &str,
) -> Result<EtsJobLink, AppError> {
    let mut connection = pool.acquire().await?;
    load_job_link_by_vtc_job_id(&mut connection, vtc_job_id).await
}

pub async fn handle_telemetry_job_event(
    app: &AppHandle,
    pool: &SqlitePool,
    payload: &TelemetryJobEventPayload,
) -> Result<(), AppError> {
    if !payload.on_job && !payload.job_finished && !payload.job_delivered {
        return Ok(());
    }

    let mut connection = begin_immediate(pool).await?;
    let result = async {
        let maybe_link = find_matching_link(&mut connection, payload).await?;
        let Some(link) = maybe_link else {
            return Ok(None);
        };
        let now = Utc::now().to_rfc3339();

        if payload.job_delivered || payload.job_finished {
            update_status_only(
                &mut connection,
                &link.link_id,
                EtsJobLinkStatus::Completed,
                Some(&now),
                None,
                None,
                None,
                None,
            )
            .await?;
            insert_audit(
                &mut connection,
                &link.link_id,
                Some(link.status),
                EtsJobLinkStatus::Completed,
                Some(&serde_json::to_string(payload).unwrap_or_default()),
            )
            .await?;
            insert_vtc_job_ledger(&mut connection, &link, payload, &now).await?;
            set_dispatcher_link_status(
                &mut connection,
                &link.vtc_job_id,
                EtsJobLinkStatus::Completed,
            )
            .await?;
            mark_dispatcher_job_completed(&mut connection, &link.vtc_job_id, &now).await?;
        } else if payload.on_job && link.status != EtsJobLinkStatus::Synced {
            update_status_only(
                &mut connection,
                &link.link_id,
                EtsJobLinkStatus::Synced,
                Some(&now),
                None,
                None,
                Some(&now),
                None,
            )
            .await?;
            insert_audit(
                &mut connection,
                &link.link_id,
                Some(link.status),
                EtsJobLinkStatus::Synced,
                Some(&serde_json::to_string(payload).unwrap_or_default()),
            )
            .await?;
            set_dispatcher_link_status(&mut connection, &link.vtc_job_id, EtsJobLinkStatus::Synced)
                .await?;
        }

        load_job_link_by_id(&mut connection, &link.link_id)
            .await
            .map(Some)
    }
    .await;

    if let Some(link) = complete_transaction(&mut connection, result).await? {
        emit_job_link_events(Some(app), &link);
    }

    Ok(())
}

async fn begin_immediate(
    pool: &SqlitePool,
) -> Result<sqlx::pool::PoolConnection<Sqlite>, AppError> {
    let mut connection = pool.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await
        .map_err(map_begin_error)?;
    Ok(connection)
}

async fn complete_transaction<T>(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    result: Result<T, AppError>,
) -> Result<T, AppError> {
    match result {
        Ok(value) => {
            sqlx::query("COMMIT")
                .execute(&mut **connection)
                .await
                .map_err(|error| AppError::new(AppErrorCode::WriteFailed, error.to_string()))?;
            Ok(value)
        }
        Err(error) => {
            if let Err(rollback_error) = sqlx::query("ROLLBACK").execute(&mut **connection).await {
                return Err(AppError::new(
                    AppErrorCode::RollbackFailed,
                    format!("{} | rollback failed: {}", error.message, rollback_error),
                ));
            }
            Err(error)
        }
    }
}

fn map_begin_error(error: sqlx::Error) -> AppError {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("locked") {
        AppError::new(AppErrorCode::LockTimeout, message)
    } else {
        AppError::new(AppErrorCode::WriteFailed, message)
    }
}

struct PreparedJobLinkUpsert<'a> {
    link_id: &'a str,
    profile_id: &'a str,
    save_id: &'a str,
    vtc_job_id: &'a str,
    offer_pointer: &'a str,
    src_company: &'a str,
    src_city: &'a str,
    dst_company: &'a str,
    dst_city: &'a str,
    cargo_id: &'a str,
    distance_km: f64,
    planned_reward: i64,
    patch_json: &'a str,
    now: &'a str,
}

async fn ensure_prepare_link_state(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    vtc_job_id: &str,
) -> Result<Option<EtsJobLink>, AppError> {
    let existing = load_job_link_by_vtc_job_id_optional(connection, vtc_job_id).await?;
    if let Some(link) = existing.as_ref() {
        if !matches!(
            link.status,
            EtsJobLinkStatus::Pending | EtsJobLinkStatus::Error | EtsJobLinkStatus::Completed
        ) {
            return Err(AppError::new(
                AppErrorCode::JobLinkConflict,
                format!(
                    "Job link already exists for {} with status {}",
                    vtc_job_id,
                    link.status.as_db()
                ),
            ));
        }
    }
    Ok(existing)
}

async fn upsert_prepared_job_link(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    existing_link: Option<&EtsJobLink>,
    input: PreparedJobLinkUpsert<'_>,
) -> Result<Option<EtsJobLinkStatus>, AppError> {
    if let Some(link) = existing_link {
        sqlx::query(
            r#"
            UPDATE ets_job_links
            SET
                profile_id = ?2,
                save_id = ?3,
                vtc_job_id = ?4,
                offer_pointer = ?5,
                job_offer_data_pointer = ?5,
                src_company = ?6,
                src_city = ?7,
                dst_company = ?8,
                dst_city = ?9,
                cargo_id = ?10,
                distance_km = ?11,
                planned_reward = ?12,
                patch_json = ?13,
                status = ?14,
                error_code = NULL,
                error_message = NULL,
                updated_at_utc = ?15,
                written_at_utc = NULL,
                requires_load_at_utc = NULL,
                synced_at_utc = NULL,
                completed_at_utc = NULL
            WHERE link_id = ?1
            "#,
        )
        .bind(input.link_id)
        .bind(input.profile_id)
        .bind(input.save_id)
        .bind(input.vtc_job_id)
        .bind(input.offer_pointer)
        .bind(input.src_company)
        .bind(input.src_city)
        .bind(input.dst_company)
        .bind(input.dst_city)
        .bind(input.cargo_id)
        .bind(input.distance_km)
        .bind(input.planned_reward)
        .bind(input.patch_json)
        .bind(EtsJobLinkStatus::Prepared.as_db())
        .bind(input.now)
        .execute(&mut **connection)
        .await?;
        return Ok(Some(link.status));
    }

    sqlx::query(
        r#"
        INSERT INTO ets_job_links (
            link_id,
            profile_id,
            save_id,
            vtc_job_id,
            offer_pointer,
            job_offer_data_pointer,
            src_company,
            src_city,
            dst_company,
            dst_city,
            cargo_id,
            distance_km,
            planned_reward,
            patch_json,
            status,
            created_at_utc,
            updated_at_utc
        )
        VALUES (
            ?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15
        )
        "#,
    )
    .bind(input.link_id)
    .bind(input.profile_id)
    .bind(input.save_id)
    .bind(input.vtc_job_id)
    .bind(input.offer_pointer)
    .bind(input.src_company)
    .bind(input.src_city)
    .bind(input.dst_company)
    .bind(input.dst_city)
    .bind(input.cargo_id)
    .bind(input.distance_km)
    .bind(input.planned_reward)
    .bind(input.patch_json)
    .bind(EtsJobLinkStatus::Prepared.as_db())
    .bind(input.now)
    .execute(&mut **connection)
    .await?;

    Ok(None)
}

pub async fn mark_dispatcher_prepare_error(
    pool: &SqlitePool,
    vtc_job_id: &str,
    error: &AppError,
) -> Result<(), AppError> {
    let mut connection = begin_immediate(pool).await?;
    let result = async {
        let existing = load_job_link_by_vtc_job_id_optional(&mut connection, vtc_job_id).await?;
        let now = Utc::now().to_rfc3339();
        let error_code = error.code.as_key();

        if let Some(link) = existing.as_ref() {
            update_job_link_error(
                &mut connection,
                &link.link_id,
                Some(link.status),
                error_code,
                &error.message,
                &now,
            )
            .await?;
        }

        sqlx::query(
            r#"
            UPDATE dispatcher_jobs
            SET
                status = 'assigned_to_save',
                ets2_job_link_status = 'error',
                last_error_code = ?2,
                last_error_message = ?3,
                updated_at_utc = ?4
            WHERE id = ?1
            "#,
        )
        .bind(vtc_job_id)
        .bind(error_code)
        .bind(&error.message)
        .bind(&now)
        .execute(&mut *connection)
        .await?;

        Ok(())
    }
    .await;

    complete_transaction(&mut connection, result).await
}

async fn update_job_link_error(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    link_id: &str,
    from_status: Option<EtsJobLinkStatus>,
    error_code: &str,
    error_message: &str,
    now: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE ets_job_links
        SET
            status = ?2,
            updated_at_utc = ?3,
            error_code = ?4,
            error_message = ?5
        WHERE link_id = ?1
        "#,
    )
    .bind(link_id)
    .bind(EtsJobLinkStatus::Error.as_db())
    .bind(now)
    .bind(error_code)
    .bind(error_message)
    .execute(&mut **connection)
    .await?;
    insert_audit(
        connection,
        link_id,
        from_status,
        EtsJobLinkStatus::Error,
        Some(
            &serde_json::json!({
                "errorCode": error_code,
                "errorMessage": error_message,
            })
            .to_string(),
        ),
    )
    .await?;
    Ok(())
}

async fn load_vtc_dispatcher_job(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    vtc_job_id: &str,
) -> Result<VtcDispatcherJob, AppError> {
    sqlx::query_as::<_, VtcDispatcherJob>(
        r#"
        SELECT
            id AS vtc_job_id,
            source_type,
            company_id,
            company_name,
            payment_tier_snapshot AS payment_tier,
            job_type,
            cargo_type,
            cargo_mass_kg,
            urgency_level,
            difficulty_level,
            equipment_type_required,
            trailer_type_required,
            origin_city,
            origin_country,
            destination_city,
            destination_country,
            distance_km AS route_distance_km,
            estimated_duration_minutes,
            base_rate_per_km,
            calculated_rate_per_km,
            total_reward,
            profile_reference,
            quicksave_reference,
            save_reference,
            route_reference
        FROM dispatcher_jobs
        WHERE id = ?1
        "#,
    )
    .bind(vtc_job_id)
    .fetch_optional(&mut **connection)
    .await?
    .ok_or_else(|| {
        AppError::new(
            AppErrorCode::InvalidToken,
            format!("Dispatcher job not found: {}", vtc_job_id),
        )
    })
}

async fn load_save_slot(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    save_id: &str,
) -> Result<EtsSaveSlot, AppError> {
    sqlx::query_as::<_, EtsSaveSlot>(
        r#"
        SELECT
            save_id,
            profile_id,
            slot_name,
            save_path,
            game_sii_path,
            is_quicksave,
            modified_at_utc,
            created_at_utc,
            updated_at_utc,
            last_loaded_at_utc
        FROM ets_saves
        WHERE save_id = ?1
        "#,
    )
    .bind(save_id)
    .fetch_optional(&mut **connection)
    .await?
    .ok_or_else(|| {
        AppError::new(
            AppErrorCode::SaveNotFound,
            format!("Save not found: {}", save_id),
        )
    })
}

async fn load_job_link_by_id(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    link_id: &str,
) -> Result<EtsJobLink, AppError> {
    let row = sqlx::query("SELECT * FROM ets_job_links WHERE link_id = ?1")
        .bind(link_id)
        .fetch_optional(&mut **connection)
        .await?
        .ok_or_else(|| {
            AppError::new(
                AppErrorCode::InvalidToken,
                format!("Link not found: {}", link_id),
            )
        })?;
    map_job_link_row(&row)
}

async fn load_job_link_by_vtc_job_id(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    vtc_job_id: &str,
) -> Result<EtsJobLink, AppError> {
    let row = sqlx::query("SELECT * FROM ets_job_links WHERE vtc_job_id = ?1")
        .bind(vtc_job_id)
        .fetch_optional(&mut **connection)
        .await?
        .ok_or_else(|| {
            AppError::new(
                AppErrorCode::InvalidToken,
                format!("No ETS job link for {}", vtc_job_id),
            )
        })?;
    map_job_link_row(&row)
}

async fn load_job_link_by_vtc_job_id_optional(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    vtc_job_id: &str,
) -> Result<Option<EtsJobLink>, AppError> {
    let row = sqlx::query("SELECT * FROM ets_job_links WHERE vtc_job_id = ?1")
        .bind(vtc_job_id)
        .fetch_optional(&mut **connection)
        .await?;
    match row {
        Some(row) => map_job_link_row(&row).map(Some),
        None => Ok(None),
    }
}

fn map_job_link_row(row: &sqlx::sqlite::SqliteRow) -> Result<EtsJobLink, AppError> {
    let status_raw = row.get::<String, _>("status");
    let patch_json = row.get::<String, _>("patch_json");
    let patch = serde_json::from_str(&patch_json)
        .map_err(|error| AppError::new(AppErrorCode::InvalidToken, error.to_string()))?;

    Ok(EtsJobLink {
        link_id: row.get("link_id"),
        profile_id: row.get("profile_id"),
        save_id: row.get("save_id"),
        vtc_job_id: row.get("vtc_job_id"),
        offer_pointer: row.try_get("offer_pointer").ok(),
        job_offer_data_pointer: row.try_get("job_offer_data_pointer").ok(),
        src_company: row.get("src_company"),
        src_city: row.get("src_city"),
        dst_company: row.get("dst_company"),
        dst_city: row.get("dst_city"),
        cargo_id: row.get("cargo_id"),
        distance_km: row.get("distance_km"),
        planned_reward: row.get("planned_reward"),
        patch,
        status: parse_status(&status_raw),
        error_code: row.try_get("error_code").ok(),
        error_message: row.try_get("error_message").ok(),
        created_at_utc: row.get("created_at_utc"),
        updated_at_utc: row.get("updated_at_utc"),
        written_at_utc: row.try_get("written_at_utc").ok(),
        requires_load_at_utc: row.try_get("requires_load_at_utc").ok(),
        synced_at_utc: row.try_get("synced_at_utc").ok(),
        completed_at_utc: row.try_get("completed_at_utc").ok(),
    })
}

fn parse_status(value: &str) -> EtsJobLinkStatus {
    match value {
        "prepared" => EtsJobLinkStatus::Prepared,
        "written" => EtsJobLinkStatus::Written,
        "requires_load" => EtsJobLinkStatus::RequiresLoad,
        "synced" => EtsJobLinkStatus::Synced,
        "completed" => EtsJobLinkStatus::Completed,
        "error" => EtsJobLinkStatus::Error,
        _ => EtsJobLinkStatus::Pending,
    }
}

async fn update_status_only(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    link_id: &str,
    status: EtsJobLinkStatus,
    written_at_utc: Option<&str>,
    offer_pointer: Option<&str>,
    job_offer_data_pointer: Option<&str>,
    synced_at_utc: Option<&str>,
    completed_at_utc: Option<&str>,
) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE ets_job_links
        SET
            status = ?2,
            updated_at_utc = ?3,
            written_at_utc = COALESCE(?4, written_at_utc),
            requires_load_at_utc = CASE WHEN ?2 = 'requires_load' THEN ?3 ELSE requires_load_at_utc END,
            offer_pointer = COALESCE(?5, offer_pointer),
            job_offer_data_pointer = COALESCE(?6, job_offer_data_pointer),
            synced_at_utc = COALESCE(?7, synced_at_utc),
            completed_at_utc = COALESCE(?8, completed_at_utc),
            error_code = NULL,
            error_message = NULL
        WHERE link_id = ?1
        "#,
    )
    .bind(link_id)
    .bind(status.as_db())
    .bind(&now)
    .bind(written_at_utc)
    .bind(offer_pointer)
    .bind(job_offer_data_pointer)
    .bind(synced_at_utc)
    .bind(completed_at_utc)
    .execute(&mut **connection)
    .await?;
    Ok(())
}

async fn set_dispatcher_link_status(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    vtc_job_id: &str,
    status: EtsJobLinkStatus,
) -> Result<(), AppError> {
    let dispatcher_status = dispatcher_status_for_link_status(status);
    sqlx::query(
        "UPDATE dispatcher_jobs SET status = ?2, ets2_job_link_status = ?3, last_error_code = NULL, last_error_message = NULL, updated_at_utc = ?4 WHERE id = ?1",
    )
    .bind(vtc_job_id)
    .bind(dispatcher_status)
    .bind(status.as_db())
    .bind(Utc::now().to_rfc3339())
    .execute(&mut **connection)
    .await?;
    Ok(())
}

fn dispatcher_status_for_link_status(status: EtsJobLinkStatus) -> &'static str {
    match status {
        EtsJobLinkStatus::Pending => "assigned_to_save",
        EtsJobLinkStatus::Prepared => "prepared",
        EtsJobLinkStatus::Written | EtsJobLinkStatus::RequiresLoad | EtsJobLinkStatus::Synced => {
            "injected"
        }
        EtsJobLinkStatus::Completed => "completed",
        EtsJobLinkStatus::Error => "failed",
    }
}

async fn mark_dispatcher_job_completed(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    vtc_job_id: &str,
    completed_at_utc: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE dispatcher_jobs
        SET status = 'completed',
            completed_at_utc = ?2,
            updated_at_utc = ?2
        WHERE id = ?1
        "#,
    )
    .bind(vtc_job_id)
    .bind(completed_at_utc)
    .execute(&mut **connection)
    .await?;
    Ok(())
}

async fn insert_audit(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    link_id: &str,
    from_status: Option<EtsJobLinkStatus>,
    to_status: EtsJobLinkStatus,
    payload_json: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO ets_job_link_audit (
            link_id,
            from_status,
            to_status,
            payload_json,
            created_at_utc
        )
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
    )
    .bind(link_id)
    .bind(from_status.map(|value| value.as_db().to_string()))
    .bind(to_status.as_db())
    .bind(payload_json)
    .bind(Utc::now().to_rfc3339())
    .execute(&mut **connection)
    .await?;
    Ok(())
}

async fn insert_vtc_job_ledger(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    link: &EtsJobLink,
    payload: &TelemetryJobEventPayload,
    now: &str,
) -> Result<(), AppError> {
    let event_type = if payload.job_delivered {
        "job_delivered"
    } else {
        "job_finished"
    };
    sqlx::query(
        r#"
        INSERT INTO vtc_job_ledger (
            link_id,
            vtc_job_id,
            event_type,
            revenue,
            payload_json,
            created_at_utc
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&link.link_id)
    .bind(&link.vtc_job_id)
    .bind(event_type)
    .bind(payload.job_delivered_revenue.max(payload.job_income))
    .bind(serde_json::to_string(payload).unwrap_or_default())
    .bind(now)
    .execute(&mut **connection)
    .await?;
    Ok(())
}

async fn find_matching_link(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    payload: &TelemetryJobEventPayload,
) -> Result<Option<EtsJobLink>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT *
        FROM ets_job_links
        WHERE status IN ('prepared', 'written', 'requires_load', 'synced')
        ORDER BY updated_at_utc DESC
        LIMIT 20
        "#,
    )
    .fetch_all(&mut **connection)
    .await?;

    let cargo = payload
        .cargo_id
        .as_deref()
        .or(payload.cargo.as_deref())
        .map(normalize_cargo_match)
        .unwrap_or_default();
    let src_company = payload
        .comp_src_id
        .as_deref()
        .or(payload.comp_src.as_deref())
        .map(normalize_company_match)
        .unwrap_or_default();
    let src_city = payload
        .city_src_id
        .as_deref()
        .or(payload.city_src.as_deref())
        .map(normalize_city_match)
        .unwrap_or_default();
    let dst_company = payload
        .comp_dst_id
        .as_deref()
        .or(payload.comp_dst.as_deref())
        .map(normalize_company_match)
        .unwrap_or_default();
    let dst_city = payload
        .city_dst_id
        .as_deref()
        .or(payload.city_dst.as_deref())
        .map(normalize_city_match)
        .unwrap_or_default();

    for row in rows {
        let link = map_job_link_row(&row)?;
        if link.cargo_id == cargo
            && link.src_company == src_company
            && link.src_city == src_city
            && link.dst_company == dst_company
            && link.dst_city == dst_city
        {
            return Ok(Some(link));
        }
    }

    Ok(None)
}

fn normalize_cargo_match(value: &str) -> String {
    let trimmed = value.trim().trim_matches('"');
    let without_prefix = trimmed.strip_prefix("cargo.").unwrap_or(trimmed);
    sii_token(without_prefix)
}

fn normalize_company_match(value: &str) -> String {
    let trimmed = value.trim().trim_matches('"');
    let without_prefix = trimmed.strip_prefix("company.volatile.").unwrap_or(trimmed);
    let company = without_prefix.split('.').next().unwrap_or(without_prefix);
    sii_token(company)
}

fn normalize_city_match(value: &str) -> String {
    let trimmed = value.trim().trim_matches('"');
    let without_prefix = trimmed.strip_prefix("city.").unwrap_or(trimmed);
    let city = without_prefix.rsplit('.').next().unwrap_or(without_prefix);
    sii_token(city)
}

fn emit_job_link_events(app: Option<&AppHandle>, link: &EtsJobLink) {
    let Some(app) = app else {
        return;
    };

    let _ = app.emit(EVT_JOB_LINK_UPDATED, link);
    let _ = app.emit(
        EVT_DISPATCHER_JOBS_UPDATED,
        serde_json::json!({
            "vtcJobId": link.vtc_job_id,
            "status": link.status,
        }),
    );
    let _ = app.emit(
        EVT_DISPATCHER_JOB_UPDATED,
        serde_json::json!({
            "jobId": link.vtc_job_id,
            "status": dispatcher_status_for_link_status(link.status),
            "ets2JobLinkStatus": link.status.as_db(),
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::create_pool;

    #[test]
    fn db_migrations_apply() {
        tauri::async_runtime::block_on(async {
            let db_path =
                std::env::temp_dir().join(format!("ets_job_links_{}.sqlite", uuid::Uuid::new_v4()));
            let pool = create_pool(&db_path).await.unwrap();
            sqlx::query("INSERT INTO ets_profiles (profile_id, profile_path, game, steam_cloud_enabled, created_at_utc, updated_at_utc) VALUES (?1, ?2, 'ets2', 0, 'now', 'now')")
                .bind("profile-1")
                .bind("C:/profiles/test")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO ets_saves (save_id, profile_id, slot_name, save_path, game_sii_path, is_quicksave, modified_at_utc, created_at_utc, updated_at_utc, last_loaded_at_utc) VALUES (?1, ?2, 'quicksave', ?3, ?4, 1, 'now', 'now', 'now', 'now')")
                .bind("save-1")
                .bind("profile-1")
                .bind("C:/profiles/test/save/quicksave")
                .bind("C:/profiles/test/save/quicksave/game.sii")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO ets_job_links (link_id, profile_id, save_id, vtc_job_id, src_company, src_city, dst_company, dst_city, cargo_id, distance_km, planned_reward, patch_json, status, created_at_utc, updated_at_utc) VALUES (?1, ?2, ?3, ?4, 'tradeaux', 'berlin', 'tradeaux', 'hamburg', 'cargo_trucks', 520, 12000, '{}', 'pending', 'now', 'now')")
                .bind("link-1")
                .bind("profile-1")
                .bind("save-1")
                .bind("vtc-1")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("UPDATE ets_job_links SET status = 'synced' WHERE link_id = 'link-1'")
                .execute(&pool)
                .await
                .unwrap();

            let status: String =
                sqlx::query_scalar("SELECT status FROM ets_job_links WHERE link_id = 'link-1'")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(status, "synced");
        });
    }
}
