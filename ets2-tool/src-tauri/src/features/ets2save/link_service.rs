use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use chrono::Utc;
use rusqlite::{Connection as RusqliteConnection, OptionalExtension};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, Sqlite, SqlitePool};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::events::{
    EVT_DISPATCHER_JOB_UPDATED, EVT_DISPATCHER_JOBS_UPDATED, EVT_JOB_LINK_UPDATED,
};
use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::features::ets2save::injector::{build_offer_patch, write_job_offer_patch};
use crate::features::ets2save::locator::resolve_last_quicksave;
use crate::features::ets2save::models::{
    DispatcherResolvedSaveLink, DispatcherSaveOfferTemplate, EtsJobLink, EtsJobLinkStatus,
    EtsJobWriteResult, EtsSaveSlot, VtcDispatcherJob,
};
use crate::features::ets2save::parser::{
    extract_job_offer_pointer, fallback_company_in_city, fallback_company_in_city_with_offers,
    find_company_block, find_job_offer_data_block, resolve_city_token, scan_save_templates,
    sii_token,
};
use crate::features::ets2save::sii_codec::decode_sii_lines;
use crate::features::ets2save::snapshot::{self, SaveSnapshotInput};
use crate::features::telemetry::events::TelemetryJobEventPayload;
use crate::shared::models::save_context::build_save_session_id;
use crate::shared::sqlite_schema::ensure_columns;
use crate::state::AppProfileState;

const RUNTIME_MIGRATIONS: [(&str, &str); 10] = [
    (
        "2026-04-06_create_ets_profiles.sql",
        include_str!("../../db/migrations/2026-04-06_create_ets_profiles.sql"),
    ),
    (
        "2026-04-06_create_ets_saves.sql",
        include_str!("../../db/migrations/2026-04-06_create_ets_saves.sql"),
    ),
    (
        "2026-04-06_create_ets_job_links.sql",
        include_str!("../../db/migrations/2026-04-06_create_ets_job_links.sql"),
    ),
    (
        "2026-04-06_create_ets_job_link_audit.sql",
        include_str!("../../db/migrations/2026-04-06_create_ets_job_link_audit.sql"),
    ),
    (
        "2026-04-06_create_vtc_job_ledger.sql",
        include_str!("../../db/migrations/2026-04-06_create_vtc_job_ledger.sql"),
    ),
    (
        "2026-04-06_create_ets2_datasets.sql",
        include_str!("../../db/migrations/2026-04-06_create_ets2_datasets.sql"),
    ),
    (
        "2026-04-06_create_ets_save_snapshot.sql",
        include_str!("../../db/migrations/2026-04-06_create_ets_save_snapshot.sql"),
    ),
    (
        "2026-04-06_add_resolved_tokens_to_ets_job_links.sql",
        include_str!("../../db/migrations/2026-04-06_add_resolved_tokens_to_ets_job_links.sql"),
    ),
    (
        "2026-04-06_add_cargo_resolution_to_ets_job_links.sql",
        include_str!("../../db/migrations/2026-04-06_add_cargo_resolution_to_ets_job_links.sql"),
    ),
    (
        "2026-04-07_add_vtc_local_persistence.sql",
        include_str!("../../db/migrations/2026-04-07_add_vtc_local_persistence.sql"),
    ),
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

    for (filename, sql) in RUNTIME_MIGRATIONS {
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
    ensure_runtime_columns(&connection)?;
    Ok(())
}

fn ensure_runtime_columns(conn: &RusqliteConnection) -> Result<(), AppError> {
    ensure_columns(
        conn,
        "ets_save_depots",
        &[
            ("discovered", "INTEGER NOT NULL DEFAULT 1"),
            ("job_offer_count", "INTEGER NOT NULL DEFAULT 0"),
        ],
    )
    .map_err(|error| AppError::new(AppErrorCode::WriteFailed, error))?;
    conn.execute(
        "UPDATE ets_save_depots SET job_offer_count = 0 WHERE job_offer_count IS NULL",
        [],
    )
    .map_err(|error| {
        AppError::new(
            AppErrorCode::WriteFailed,
            format!("backfill ets_save_depots job_offer_count failed: {}", error),
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
    let lines = decode_sii_lines(std::path::Path::new(&save_slot.game_sii_path))?;
    let scan = scan_save_templates(&lines);
    let dispatcher_job = {
        let mut read_connection = pool.acquire().await?;
        load_vtc_dispatcher_job(&mut read_connection, vtc_job_id).await?
    };
    let save_session_id = dispatcher_job
        .save_session_id
        .clone()
        .or_else(|| {
            build_save_session_id(
                Some(profile.profile_path.as_str()),
                Some(save_slot.save_path.as_str()),
            )
        })
        .ok_or_else(|| {
            AppError::new(
                AppErrorCode::InvalidToken,
                format!("save_session_id missing for {}", vtc_job_id),
            )
        })?;
    let snapshot_input = SaveSnapshotInput {
        save_session_id: save_session_id.clone(),
        profile_reference: dispatcher_job.profile_reference.clone(),
        save_reference: dispatcher_job
            .save_reference
            .clone()
            .or_else(|| Some(save_slot.save_path.clone())),
        quicksave_reference: dispatcher_job
            .quicksave_reference
            .clone()
            .or_else(|| Some(save_slot.save_path.clone())),
    };
    let snapshot = snapshot::snapshot_refresh(app, pool, snapshot_input).await?;
    let cargo_resolution =
        resolve_prepare_cargo_token(&dispatcher_job, &snapshot.transported_cargo_tokens)?;
    crate::dev_log!(
        "[ets2save] cargo resolve job={} requested={} resolved={} mode={} source={} snapshot_valid={} snapshot_cargo_count={}",
        vtc_job_id,
        cargo_resolution.requested_cargo_token,
        cargo_resolution.resolved_cargo_token,
        cargo_resolution.cargo_resolution_mode,
        cargo_resolution.cargo_validation_source,
        cargo_resolution.cargo_valid_for_snapshot,
        snapshot.transported_cargo_tokens.len()
    );

    let mut connection = begin_immediate(pool).await?;
    let result = async {
        let resolved =
            resolve_prepare_save_mapping(&mut connection, &dispatcher_job, &snapshot).await?;
        let company_block = find_company_block(
            &lines,
            &resolved.resolved_src_company_token,
            &resolved.resolved_src_city_token,
        )?;
        let offer_pointer = extract_job_offer_pointer(&lines, company_block)?;
        let offer_range = find_job_offer_data_block(&lines, &offer_pointer)?;
        let existing_link = ensure_prepare_link_state(&mut connection, vtc_job_id).await?;
        let link_id = existing_link
            .as_ref()
            .map(|link| link.link_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let mut patch = build_offer_patch(
            &dispatcher_job,
            &lines,
            offer_range,
            &resolved.resolved_dst_company_token,
            &link_id,
        );
        patch.cargo = format!("cargo.{}", cargo_resolution.resolved_cargo_token.clone());
        patch.target = format!(
            "{}.{}",
            sii_token(&resolved.resolved_dst_company_token),
            resolved.resolved_dst_city_token
        );
        let cargo_id = cargo_resolution.resolved_cargo_token.clone();
        let now = Utc::now().to_rfc3339();
        let patch_json = serde_json::to_string(&patch)
            .map_err(|error| AppError::new(AppErrorCode::WriteFailed, error.to_string()))?;
        let template = build_dispatcher_save_offer_template(
            &dispatcher_job,
            &resolved,
            &cargo_resolution,
            &scan,
            &offer_pointer,
            &patch,
            &snapshot,
        );
        let template_json = serde_json::to_string(&template)
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
                src_company: &resolved.resolved_src_company_token,
                src_city: &resolved.resolved_src_city_token,
                dst_company: &resolved.resolved_dst_company_token,
                dst_city: &resolved.resolved_dst_city_token,
                resolved_source_company_token: &resolved.resolved_src_company_token,
                resolved_source_city_token: &resolved.resolved_src_city_token,
                resolved_target_company_token: &resolved.resolved_dst_company_token,
                resolved_target_city_token: &resolved.resolved_dst_city_token,
                requested_source_company_token: &sii_token(&dispatcher_job.company_id),
                requested_source_city_token: &sii_token(&dispatcher_job.origin_city),
                requested_target_company_token: &sii_token(&dispatcher_job.company_id),
                requested_target_city_token: &sii_token(&dispatcher_job.destination_city),
                trailer_definition_token: patch.trailer_definition.as_deref(),
                trailer_variant_token: patch.trailer_variant.as_deref(),
                company_truck_mode: if patch.company_truck {
                    "company_truck"
                } else {
                    "own_truck"
                },
                requested_cargo_token: &cargo_resolution.requested_cargo_token,
                resolved_cargo_token: &cargo_resolution.resolved_cargo_token,
                cargo_resolution_mode: &cargo_resolution.cargo_resolution_mode,
                cargo_validation_source: &cargo_resolution.cargo_validation_source,
                cargo_valid_for_snapshot: cargo_resolution.cargo_valid_for_snapshot,
                cargo_id: &cargo_id,
                distance_km: dispatcher_job.route_distance_km,
                planned_reward: dispatcher_job.total_reward,
                patch_json: &patch_json,
                save_offer_template_json: &template_json,
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
) -> Result<EtsJobWriteResult, AppError> {
    write_job_to_quicksave(Some(app), pool, link_id).await
}

pub async fn write_job_to_quicksave(
    app: Option<&AppHandle>,
    pool: &SqlitePool,
    link_id: &str,
) -> Result<EtsJobWriteResult, AppError> {
    let mut connection = begin_immediate(pool).await?;
    let result = async {
        let link = load_job_link_by_id(&mut connection, link_id).await?;
        let save_slot = load_save_slot(&mut connection, &link.save_id).await?;
        let before_sha256 = file_sha256_hex(std::path::Path::new(&save_slot.game_sii_path))?;
        let pointers = write_job_offer_patch(
            std::path::Path::new(&save_slot.game_sii_path),
            &link.src_company,
            &link.src_city,
            &link.patch,
        )?;
        let after_sha256 = file_sha256_hex(std::path::Path::new(&save_slot.game_sii_path))?;
        let now = Utc::now().to_rfc3339();
        let validation = pointers.validation.clone();
        let expected_load_path = Some(save_slot.save_path.clone());
        let load_path_warning = link
            .save_offer_template
            .as_ref()
            .and_then(|template| template.quicksave_reference.clone())
            .and_then(|expected| {
                if expected != save_slot.save_path {
                    Some(format!(
                        "quicksave_path_mismatch expected={} actual={}",
                        expected, save_slot.save_path
                    ))
                } else {
                    None
                }
            });

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
        let backup_payload = pointers.backup_path.as_ref().map(|path| {
            serde_json::json!({ "backupPath": path.display().to_string() }).to_string()
        });
        insert_audit(
            &mut connection,
            link_id,
            Some(EtsJobLinkStatus::Written),
            EtsJobLinkStatus::RequiresLoad,
            backup_payload.as_deref(),
        )
        .await?;
        set_dispatcher_link_status(
            &mut connection,
            &link.vtc_job_id,
            EtsJobLinkStatus::RequiresLoad,
        )
        .await?;

        let updated_link = load_job_link_by_id(&mut connection, link_id).await?;
        Ok(EtsJobWriteResult {
            link: updated_link,
            save_path: save_slot.game_sii_path,
            backup_path: pointers
                .backup_path
                .as_ref()
                .map(|path| path.display().to_string()),
            before_sha256,
            after_sha256,
            write_mode: "overwrite_existing_offer".to_string(),
            job_info_updated: pointers.job_info_updated,
            post_write_valid: validation.valid,
            validation: validation.clone(),
            post_write_validated: validation.valid,
            company_block_found_after_write: validation.company_block_found,
            offer_pointer_found_after_write: validation.offer_pointer_found,
            job_offer_data_found_after_write: validation.offer_data_found,
            cargo_written_token: validation.written_cargo.clone().unwrap_or_default(),
            target_written_token: validation.written_target.clone().unwrap_or_default(),
            shortest_distance_written: validation.written_shortest_distance_km,
            expiration_time_written: validation.written_expiration_time,
            job_info_status: if link.patch.job_info_unit.is_some() {
                "updated".to_string()
            } else {
                "not_applicable".to_string()
            },
            validation_error_code: validation.validation_error_code.clone(),
            validation_error_message: validation.validation_error.clone(),
            offer_slot_index: Some(pointers.offer_slot_index as i64),
            offer_slot_pointer: Some(pointers.offer_pointer),
            expected_load_path,
            load_path_warning,
        })
    }
    .await;

    complete_transaction(&mut connection, result)
        .await
        .map(|link| {
            emit_job_link_events(app, &link.link);
            link
        })
}

fn file_sha256_hex(path: &std::path::Path) -> Result<String, AppError> {
    let bytes = fs::read(path).map_err(|error| {
        AppError::new(
            AppErrorCode::WriteFailed,
            format!("read for sha failed {}: {}", path.display(), error),
        )
    })?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
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

        if payload.job_delivered {
            update_status_only(
                &mut connection,
                &link.link_id,
                EtsJobLinkStatus::Completed,
                None,
                None,
                None,
                None,
                Some(&now),
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
        } else if payload.job_cancelled || payload.job_finished {
            update_status_only(
                &mut connection,
                &link.link_id,
                EtsJobLinkStatus::Error,
                None,
                None,
                None,
                None,
                Some(&now),
            )
            .await?;
            insert_audit(
                &mut connection,
                &link.link_id,
                Some(link.status),
                EtsJobLinkStatus::Error,
                Some(&serde_json::to_string(payload).unwrap_or_default()),
            )
            .await?;
            insert_vtc_job_ledger(&mut connection, &link, payload, &now).await?;
            mark_dispatcher_job_terminal(
                &mut connection,
                &link.vtc_job_id,
                if payload.job_cancelled {
                    "cancelled"
                } else {
                    "failed"
                },
                &now,
            )
            .await?;
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
    resolved_source_company_token: &'a str,
    resolved_source_city_token: &'a str,
    resolved_target_company_token: &'a str,
    resolved_target_city_token: &'a str,
    requested_source_company_token: &'a str,
    requested_source_city_token: &'a str,
    requested_target_company_token: &'a str,
    requested_target_city_token: &'a str,
    trailer_definition_token: Option<&'a str>,
    trailer_variant_token: Option<&'a str>,
    company_truck_mode: &'a str,
    requested_cargo_token: &'a str,
    resolved_cargo_token: &'a str,
    cargo_resolution_mode: &'a str,
    cargo_validation_source: &'a str,
    cargo_valid_for_snapshot: bool,
    cargo_id: &'a str,
    distance_km: f64,
    planned_reward: i64,
    patch_json: &'a str,
    save_offer_template_json: &'a str,
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
                resolved_source_company_token = ?10,
                resolved_source_city_token = ?11,
                resolved_target_company_token = ?12,
                resolved_target_city_token = ?13,
                requested_source_company_token = ?14,
                requested_source_city_token = ?15,
                requested_target_company_token = ?16,
                requested_target_city_token = ?17,
                trailer_definition_token = ?18,
                trailer_variant_token = ?19,
                company_truck_mode = ?20,
                requested_cargo_token = ?21,
                resolved_cargo_token = ?22,
                cargo_resolution_mode = ?23,
                cargo_validation_source = ?24,
                cargo_valid_for_snapshot = ?25,
                cargo_id = ?26,
                distance_km = ?27,
                planned_reward = ?28,
                patch_json = ?29,
                save_offer_template_json = ?30,
                status = ?31,
                error_code = NULL,
                error_message = NULL,
                updated_at_utc = ?32,
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
        .bind(input.resolved_source_company_token)
        .bind(input.resolved_source_city_token)
        .bind(input.resolved_target_company_token)
        .bind(input.resolved_target_city_token)
        .bind(input.requested_source_company_token)
        .bind(input.requested_source_city_token)
        .bind(input.requested_target_company_token)
        .bind(input.requested_target_city_token)
        .bind(input.trailer_definition_token)
        .bind(input.trailer_variant_token)
        .bind(input.company_truck_mode)
        .bind(input.requested_cargo_token)
        .bind(input.resolved_cargo_token)
        .bind(input.cargo_resolution_mode)
        .bind(input.cargo_validation_source)
        .bind(if input.cargo_valid_for_snapshot {
            1_i64
        } else {
            0_i64
        })
        .bind(input.cargo_id)
        .bind(input.distance_km)
        .bind(input.planned_reward)
        .bind(input.patch_json)
        .bind(input.save_offer_template_json)
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
            resolved_source_company_token,
            resolved_source_city_token,
            resolved_target_company_token,
            resolved_target_city_token,
            requested_source_company_token,
            requested_source_city_token,
            requested_target_company_token,
            requested_target_city_token,
            trailer_definition_token,
            trailer_variant_token,
            company_truck_mode,
            requested_cargo_token,
            resolved_cargo_token,
            cargo_resolution_mode,
            cargo_validation_source,
            cargo_valid_for_snapshot,
            cargo_id,
            distance_km,
            planned_reward,
            patch_json,
            save_offer_template_json,
            status,
            created_at_utc,
            updated_at_utc
        )
        VALUES (
            ?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33
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
    .bind(input.resolved_source_company_token)
    .bind(input.resolved_source_city_token)
    .bind(input.resolved_target_company_token)
    .bind(input.resolved_target_city_token)
    .bind(input.requested_source_company_token)
    .bind(input.requested_source_city_token)
    .bind(input.requested_target_company_token)
    .bind(input.requested_target_city_token)
    .bind(input.trailer_definition_token)
    .bind(input.trailer_variant_token)
    .bind(input.company_truck_mode)
    .bind(input.requested_cargo_token)
    .bind(input.resolved_cargo_token)
    .bind(input.cargo_resolution_mode)
    .bind(input.cargo_validation_source)
    .bind(if input.cargo_valid_for_snapshot { 1_i64 } else { 0_i64 })
    .bind(input.cargo_id)
    .bind(input.distance_km)
    .bind(input.planned_reward)
    .bind(input.patch_json)
    .bind(input.save_offer_template_json)
    .bind(EtsJobLinkStatus::Prepared.as_db())
    .bind(input.now)
    .bind(input.now)
    .execute(&mut **connection)
    .await?;

    Ok(None)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SaveCompanyBlock {
    company_token: String,
    city_token: String,
    job_offer_count: i64,
}

#[derive(Debug, Clone)]
struct PrepareSaveMapping {
    resolved_src_company_token: String,
    resolved_src_city_token: String,
    resolved_dst_company_token: String,
    resolved_dst_city_token: String,
    resolution_mode: String,
    candidate_cities: Vec<String>,
}

#[derive(Debug, Clone)]
struct CargoResolution {
    requested_cargo_token: String,
    resolved_cargo_token: String,
    cargo_resolution_mode: String,
    cargo_validation_source: String,
    cargo_valid_for_snapshot: bool,
}

async fn resolve_prepare_save_mapping(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    dispatcher_job: &VtcDispatcherJob,
    snapshot: &snapshot::SaveSnapshotDto,
) -> Result<PrepareSaveMapping, AppError> {
    let requested_company_token = sii_token(&dispatcher_job.company_id);
    let requested_city_token = sii_token(&dispatcher_job.origin_city);
    let requested_destination_city_token = sii_token(&dispatcher_job.destination_city);
    let mut all_depots = Vec::<SaveCompanyBlock>::new();
    let depots_by_city = snapshot::depots_by_city(snapshot);
    for depot in &snapshot.depots {
        all_depots.push(SaveCompanyBlock {
            company_token: depot.company_token.clone(),
            city_token: depot.city_token.clone(),
            job_offer_count: depot.job_offer_count,
        });
    }
    let depot_index = crate::features::ets2save::parser::SaveDepotIndex {
        depots_by_city: depots_by_city.clone(),
        all_depots: all_depots
            .iter()
            .map(|item| (item.company_token.clone(), item.city_token.clone()))
            .collect(),
    };
    let available_blocks = all_depots;
    let source_city_resolution = resolve_city_token(&requested_city_token, &depot_index);
    let resolved_requested_city_token = source_city_resolution
        .as_ref()
        .map(|value| value.token.clone())
        .unwrap_or_else(|| requested_city_token.clone());
    let source_city_candidates = source_city_resolution
        .as_ref()
        .map(|value| value.candidates.clone())
        .unwrap_or_default();
    let destination_city_resolution =
        resolve_city_token(&requested_destination_city_token, &depot_index);
    let resolved_destination_city_token = destination_city_resolution
        .as_ref()
        .map(|value| value.token.clone())
        .unwrap_or_else(|| requested_destination_city_token.clone());

    if let Some(exact) = available_blocks.iter().find(|block| {
        block.company_token == requested_company_token
            && block.city_token == resolved_requested_city_token
    }) {
        if exact.job_offer_count > 0 {
            return Ok(PrepareSaveMapping {
                resolved_src_company_token: exact.company_token.clone(),
                resolved_src_city_token: exact.city_token.clone(),
                resolved_dst_company_token: exact.company_token.clone(),
                resolved_dst_city_token: resolved_destination_city_token.clone(),
                resolution_mode: if requested_city_token == resolved_requested_city_token {
                    "exact".to_string()
                } else {
                    "city_alias".to_string()
                },
                candidate_cities: source_city_candidates.clone(),
            });
        }
    }

    let company_candidates = load_company_token_candidates(
        connection,
        &dispatcher_job.company_id,
        &dispatcher_job.company_name,
    )
    .await?;
    let city_candidates =
        load_city_token_candidates(connection, &dispatcher_job.origin_city).await?;
    let destination_city_candidates =
        load_city_token_candidates(connection, &dispatcher_job.destination_city).await?;

    if let Some(mapped) = available_blocks.iter().find(|block| {
        company_candidates.contains(&block.company_token)
            && block.city_token == resolved_requested_city_token
    }) {
        if mapped.job_offer_count > 0 {
            return Ok(PrepareSaveMapping {
                resolved_src_company_token: mapped.company_token.clone(),
                resolved_src_city_token: mapped.city_token.clone(),
                resolved_dst_company_token: mapped.company_token.clone(),
                resolved_dst_city_token: pick_destination_city_token(
                    &available_blocks,
                    &destination_city_candidates,
                    &resolved_destination_city_token,
                ),
                resolution_mode: if requested_city_token == resolved_requested_city_token {
                    "exact".to_string()
                } else {
                    "city_alias".to_string()
                },
                candidate_cities: source_city_candidates.clone(),
            });
        }
    }

    let available_company_blocks_in_city = available_blocks
        .iter()
        .filter(|block| block.city_token == resolved_requested_city_token)
        .map(|block| format!("{}.{}", block.company_token, block.city_token))
        .collect::<Vec<_>>();

    if !available_company_blocks_in_city.is_empty() {
        let snapshot_depots_in_city = snapshot
            .depots
            .iter()
            .filter(|depot| depot.city_token == resolved_requested_city_token)
            .map(|depot| crate::features::ets2save::models::SaveDepotBlock {
                unit_token: depot.depot_key.clone(),
                company_token: depot.company_token.clone(),
                city_token: depot.city_token.clone(),
                permanent_data: None,
                job_offer_count: depot.job_offer_count.max(0) as usize,
                job_offers: Vec::new(),
            })
            .collect::<Vec<_>>();
        let selected_company = fallback_company_in_city_with_offers(
            &snapshot_depots_in_city,
            &resolved_requested_city_token,
        )
        .or_else(|| fallback_company_in_city(&depot_index, &resolved_requested_city_token))
        .unwrap_or_else(|| available_blocks[0].company_token.clone());
        let selected = available_blocks
            .iter()
            .find(|block| {
                block.city_token == resolved_requested_city_token
                    && block.company_token == selected_company
            })
            .unwrap_or(&available_blocks[0]);
        if selected.job_offer_count <= 0 {
            let depot_count = available_company_blocks_in_city.len();
            let offerless_count = available_blocks
                .iter()
                .filter(|block| {
                    block.city_token == resolved_requested_city_token && block.job_offer_count <= 0
                })
                .count();
            return Err(AppError::new(
                AppErrorCode::CompanyHasNoJobOffersInCity,
                format!(
                    "No host depot with job offers in current city | city={} | depot_count={} | offerless_count={} | suggestion=economy_reset_or_sleep",
                    resolved_requested_city_token, depot_count, offerless_count
                ),
            ));
        }
        crate::dev_log!(
            "[ets2save] prepare remap fallback: {}.{} -> {}.{}",
            requested_company_token,
            resolved_requested_city_token,
            selected.company_token,
            selected.city_token
        );
        return Ok(PrepareSaveMapping {
            resolved_src_company_token: selected.company_token.clone(),
            resolved_src_city_token: selected.city_token.clone(),
            resolved_dst_company_token: selected.company_token.clone(),
            resolved_dst_city_token: pick_destination_city_token(
                &available_blocks,
                &destination_city_candidates,
                &resolved_destination_city_token,
            ),
            resolution_mode: "fallback_host".to_string(),
            candidate_cities: source_city_candidates.clone(),
        });
    }

    let possible_company_matches = available_blocks
        .iter()
        .filter(|block| {
            block.company_token.contains(&requested_company_token)
                || requested_company_token.contains(&block.company_token)
        })
        .map(|block| block.company_token.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let possible_city_matches = available_blocks
        .iter()
        .filter(|block| {
            block.city_token.contains(&requested_city_token)
                || resolved_requested_city_token.contains(&block.city_token)
                || city_candidates.contains(&block.city_token)
        })
        .map(|block| block.city_token.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let requested_company_block = format!(
        "company.volatile.{}.{}",
        requested_company_token, resolved_requested_city_token
    );
    let available_company_blocks = available_blocks
        .iter()
        .take(24)
        .map(|block| format!("{}.{}", block.company_token, block.city_token))
        .collect::<Vec<_>>();

    Err(AppError::new(
        AppErrorCode::CompanyNotFoundInSave,
        format!(
            "Requested company depot not found in current save | requested_company_token={} | requested_city_token={} | resolved_city_token={} | requested_company_block={} | candidate_cities=[{}] | available_company_blocks_in_city=[{}] | possible_company_matches=[{}] | possible_city_matches=[{}] | available_company_blocks_sample=[{}]",
            requested_company_token,
            requested_city_token,
            resolved_requested_city_token,
            requested_company_block,
            source_city_candidates.join(", "),
            available_company_blocks_in_city.join(", "),
            possible_company_matches.join(", "),
            possible_city_matches.join(", "),
            available_company_blocks.join(", ")
        ),
    ))
}

fn resolve_prepare_cargo_token(
    dispatcher_job: &VtcDispatcherJob,
    transported_cargo_tokens: &[String],
) -> Result<CargoResolution, AppError> {
    let requested = sii_token(&dispatcher_job.cargo_type);
    let mut snapshot_tokens = transported_cargo_tokens
        .iter()
        .map(|item| sii_token(item))
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    snapshot_tokens.sort();
    snapshot_tokens.dedup();

    if snapshot_tokens.contains(&requested) {
        return Ok(CargoResolution {
            requested_cargo_token: requested.clone(),
            resolved_cargo_token: requested,
            cargo_resolution_mode: "direct_snapshot_match".to_string(),
            cargo_validation_source: "ets_save_transport_cargo".to_string(),
            cargo_valid_for_snapshot: true,
        });
    }

    let category_candidates = dispatcher_category_candidates(&requested);
    if snapshot_tokens.is_empty() {
        if let Some(first_category_token) = category_candidates.first() {
            return Ok(CargoResolution {
                requested_cargo_token: requested,
                resolved_cargo_token: (*first_category_token).to_string(),
                cargo_resolution_mode: "category_default_without_snapshot".to_string(),
                cargo_validation_source: "snapshot_unavailable".to_string(),
                cargo_valid_for_snapshot: false,
            });
        }
        if !requested.is_empty() {
            return Ok(CargoResolution {
                requested_cargo_token: requested.clone(),
                resolved_cargo_token: requested,
                cargo_resolution_mode: "direct_without_snapshot".to_string(),
                cargo_validation_source: "snapshot_unavailable".to_string(),
                cargo_valid_for_snapshot: false,
            });
        }
    }

    if !category_candidates.is_empty() {
        if let Some(found) = category_candidates
            .iter()
            .find(|token| snapshot_tokens.contains(&token.to_string()))
        {
            return Ok(CargoResolution {
                requested_cargo_token: requested,
                resolved_cargo_token: (*found).to_string(),
                cargo_resolution_mode: "category_to_snapshot_token".to_string(),
                cargo_validation_source: "ets_save_transport_cargo".to_string(),
                cargo_valid_for_snapshot: true,
            });
        }

        if let Some(fallback) = snapshot_tokens.first() {
            return Ok(CargoResolution {
                requested_cargo_token: requested,
                resolved_cargo_token: fallback.clone(),
                cargo_resolution_mode: "category_fallback_snapshot_first".to_string(),
                cargo_validation_source: "ets_save_transport_cargo".to_string(),
                cargo_valid_for_snapshot: true,
            });
        }
    }

    let sample = snapshot_tokens
        .iter()
        .take(12)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    Err(AppError::new(
        AppErrorCode::InvalidToken,
        format!(
            "cargo_not_valid_for_snapshot: requested_cargo_token={} | resolved_cargo_token=none | resolution_mode=unresolved | validation_source=ets_save_transport_cargo | snapshot_cargo_sample=[{}]",
            requested, sample
        ),
    ))
}

fn dispatcher_category_candidates(category: &str) -> &'static [&'static str] {
    match category {
        "standard" => &[
            "apples",
            "beans",
            "electronics",
            "furniture",
            "paper",
            "hardware",
            "aircond",
        ],
        "fragile" => &["electronics", "med_equip", "glass", "aircond", "radiators"],
        "adr" | "hazardous" => &[
            "acetylene",
            "chlorine",
            "hydrogen",
            "potassium",
            "cyanide",
            "explosives",
            "dynamite",
        ],
        "liquid_food" | "refrigerated" => &[
            "olive_oil_t",
            "conc_juice_t",
            "coconut_oil",
            "milk",
            "beverages",
        ],
        "heavy" | "oversize" => &[
            "locomotive",
            "helicopter",
            "diesel_gen",
            "tractors",
            "jcb_3cx",
            "volvo_l120h",
        ],
        "valuable" => &["air_mails", "med_equip", "electronics", "banknotes"],
        "machinery" => &["diesel_gen", "aircond", "volvo_l120h", "tractors"],
        "retail" => &["apples", "beans", "paper", "electronics", "furniture"],
        _ => &[],
    }
}

fn pick_destination_city_token(
    available_blocks: &[SaveCompanyBlock],
    destination_city_candidates: &BTreeSet<String>,
    requested_destination_city_token: &str,
) -> String {
    if available_blocks
        .iter()
        .any(|block| block.city_token == requested_destination_city_token)
    {
        return requested_destination_city_token.to_string();
    }
    if let Some(found) = available_blocks
        .iter()
        .find(|block| destination_city_candidates.contains(&block.city_token))
    {
        return found.city_token.clone();
    }
    requested_destination_city_token.to_string()
}

fn list_save_company_blocks(lines: &[String]) -> Vec<SaveCompanyBlock> {
    let mut blocks = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        let Some(without_prefix) = trimmed.strip_prefix("company : company.volatile.") else {
            continue;
        };
        let unit = without_prefix
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_end_matches('{')
            .trim();
        if unit.is_empty() {
            continue;
        }
        let mut parts = unit.splitn(2, '.');
        let company = parts.next().unwrap_or_default().trim();
        let city = parts.next().unwrap_or_default().trim();
        if company.is_empty() || city.is_empty() {
            continue;
        }
        blocks.push(SaveCompanyBlock {
            company_token: sii_token(company),
            city_token: sii_token(city),
            job_offer_count: 0,
        });
    }
    blocks
}

async fn load_company_token_candidates(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    requested_company_id: &str,
    requested_company_name: &str,
) -> Result<BTreeSet<String>, AppError> {
    let requested_id_token = sii_token(requested_company_id);
    let requested_name_token = sii_token(requested_company_name);
    let rows = sqlx::query(
        r#"
        SELECT id, game_token, name_en, name_local, aliases_json
        FROM ets2_companies
        "#,
    )
    .fetch_all(&mut **connection)
    .await?;

    let mut candidates = BTreeSet::new();
    candidates.insert(requested_id_token.clone());
    candidates.insert(requested_name_token.clone());

    for row in rows {
        let id: String = row.get("id");
        let game_token: String = row.get("game_token");
        let name_en: Option<String> = row.get("name_en");
        let name_local: Option<String> = row.get("name_local");
        let aliases_json: Option<String> = row.get("aliases_json");
        let aliases = parse_json_aliases(aliases_json.as_deref().unwrap_or("[]"));
        let row_tokens = row_string_tokens(
            [
                id.as_str(),
                game_token.as_str(),
                name_en.as_deref().unwrap_or_default(),
                name_local.as_deref().unwrap_or_default(),
            ],
            &aliases,
        );
        if row_tokens.contains(&requested_id_token) || row_tokens.contains(&requested_name_token) {
            candidates.insert(sii_token(&game_token));
            candidates.insert(sii_token(&id));
        }
    }

    Ok(candidates)
}

async fn load_city_token_candidates(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    requested_city: &str,
) -> Result<BTreeSet<String>, AppError> {
    let requested_city_token = sii_token(requested_city);
    let rows = sqlx::query(
        r#"
        SELECT id, game_token, name_en, name_local, aliases_json
        FROM ets2_cities
        "#,
    )
    .fetch_all(&mut **connection)
    .await?;

    let mut candidates = BTreeSet::new();
    candidates.insert(requested_city_token.clone());

    for row in rows {
        let id: String = row.get("id");
        let game_token: String = row.get("game_token");
        let name_en: Option<String> = row.get("name_en");
        let name_local: Option<String> = row.get("name_local");
        let aliases_json: Option<String> = row.get("aliases_json");
        let aliases = parse_json_aliases(aliases_json.as_deref().unwrap_or("[]"));
        let row_tokens = row_string_tokens(
            [
                id.as_str(),
                game_token.as_str(),
                name_en.as_deref().unwrap_or_default(),
                name_local.as_deref().unwrap_or_default(),
            ],
            &aliases,
        );
        if row_tokens.contains(&requested_city_token) {
            candidates.insert(sii_token(&game_token));
            candidates.insert(sii_token(&id));
        }
    }

    Ok(candidates)
}

fn row_string_tokens<'a, I>(base_fields: I, aliases: &[String]) -> BTreeSet<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut tokens = BTreeSet::new();
    for value in base_fields {
        let token = sii_token(value);
        if !token.is_empty() {
            tokens.insert(token);
        }
    }
    for alias in aliases {
        let token = sii_token(alias);
        if !token.is_empty() {
            tokens.insert(token);
        }
    }
    tokens
}

fn parse_json_aliases(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

fn build_dispatcher_save_offer_template(
    job: &VtcDispatcherJob,
    resolved: &PrepareSaveMapping,
    cargo_resolution: &CargoResolution,
    scan: &crate::features::ets2save::parser::SaveTemplateScan,
    offer_pointer: &str,
    patch: &crate::features::ets2save::models::EtsJobOfferPatch,
    snapshot: &snapshot::SaveSnapshotDto,
) -> DispatcherSaveOfferTemplate {
    let offer_data = scan.job_offer_data.get(offer_pointer);
    let resolved_source_depot_block = format!(
        "company.volatile.{}.{}",
        resolved.resolved_src_company_token, resolved.resolved_src_city_token
    );
    let job_info = scan.job_info_units.first().cloned();

    DispatcherSaveOfferTemplate {
        dispatcher_job_id: job.vtc_job_id.clone(),
        source_type: job.source_type.clone(),
        job_type: job.job_type.clone(),
        company_id: job.company_id.clone(),
        company_token: sii_token(&job.company_id),
        company_name: job.company_name.clone(),
        source_city_token: resolved.resolved_src_city_token.clone(),
        source_city_name: job.origin_city.clone(),
        source_country_token: Some(sii_token(&job.origin_country)),
        target_city_token: resolved.resolved_dst_city_token.clone(),
        target_city_name: job.destination_city.clone(),
        target_country_token: Some(sii_token(&job.destination_country)),
        target_company_token: resolved.resolved_dst_company_token.clone(),
        target_company_name: None,
        cargo_token: cargo_resolution.resolved_cargo_token.clone(),
        requested_cargo_token: Some(cargo_resolution.requested_cargo_token.clone()),
        resolved_cargo_token: Some(cargo_resolution.resolved_cargo_token.clone()),
        cargo_resolution_mode: Some(cargo_resolution.cargo_resolution_mode.clone()),
        cargo_validation_source: Some(cargo_resolution.cargo_validation_source.clone()),
        cargo_valid_for_snapshot: Some(cargo_resolution.cargo_valid_for_snapshot),
        cargo_name: Some(job.cargo_type.clone()),
        trailer_variant_token: patch.trailer_variant.clone(),
        trailer_definition_token: patch.trailer_definition.clone(),
        company_truck_token: offer_data.and_then(|value| value.company_truck.clone()),
        company_truck: patch.company_truck,
        shortest_distance_km: patch.shortest_distance_km,
        urgency: patch.urgency,
        ferry_time: patch.ferry_time,
        ferry_price: patch.ferry_price,
        units_count: patch.units_count,
        fill_ratio: patch.fill_ratio,
        trailer_place: patch.trailer_place,
        expiration_time: patch.expiration_time,
        planned_distance_km: patch.shortest_distance_km,
        save_reference: job.save_reference.clone(),
        quicksave_reference: job.quicksave_reference.clone(),
        save_session_id: job.save_session_id.clone(),
        ets2_job_link_status: "prepared".to_string(),
        dispatcher_status: job
            .dispatcher_status
            .clone()
            .unwrap_or_else(|| "prepared".to_string()),
        last_error_code: job.last_error_code.clone(),
        last_error_message: job.last_error_message.clone(),
        resolved: DispatcherResolvedSaveLink {
            resolution_mode: resolved.resolution_mode.clone(),
            requested_source_company_token: sii_token(&job.company_id),
            requested_source_city_token: sii_token(&job.origin_city),
            requested_target_city_token: sii_token(&job.destination_city),
            resolved_source_depot_block,
            resolved_source_company_token: resolved.resolved_src_company_token.clone(),
            resolved_source_city_token: resolved.resolved_src_city_token.clone(),
            resolved_target_company_token: resolved.resolved_dst_company_token.clone(),
            resolved_target_city_token: resolved.resolved_dst_city_token.clone(),
            resolved_offer_pointer: offer_pointer.to_string(),
            resolved_job_offer_data_pointer: offer_pointer.to_string(),
            resolved_job_info_pointer: job_info.as_ref().map(|value| value.pointer.clone()),
        },
        job_info,
        companies_index: scan.companies_index.clone(),
        visited_cities: snapshot.visited_city_tokens.clone(),
        source_city_visited: snapshot
            .visited_city_tokens
            .iter()
            .any(|city| sii_token(city) == resolved.resolved_src_city_token),
        target_city_visited: snapshot
            .visited_city_tokens
            .iter()
            .any(|city| sii_token(city) == resolved.resolved_dst_city_token),
        depots_by_city: snapshot::depots_by_city(snapshot),
    }
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
            save_session_id,
            route_reference,
            status AS dispatcher_status,
            last_error_code,
            last_error_message
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
    let save_offer_template = row
        .try_get::<Option<String>, _>("save_offer_template_json")
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<DispatcherSaveOfferTemplate>(&raw).ok());

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
        resolved_source_company_token: row.try_get("resolved_source_company_token").ok(),
        resolved_source_city_token: row.try_get("resolved_source_city_token").ok(),
        resolved_target_company_token: row.try_get("resolved_target_company_token").ok(),
        resolved_target_city_token: row.try_get("resolved_target_city_token").ok(),
        requested_cargo_token: row.try_get("requested_cargo_token").ok(),
        resolved_cargo_token: row.try_get("resolved_cargo_token").ok(),
        cargo_resolution_mode: row.try_get("cargo_resolution_mode").ok(),
        cargo_validation_source: row.try_get("cargo_validation_source").ok(),
        cargo_valid_for_snapshot: row
            .try_get::<i64, _>("cargo_valid_for_snapshot")
            .ok()
            .map(|value| value != 0),
        cargo_id: row.get("cargo_id"),
        distance_km: row.get("distance_km"),
        planned_reward: row.get("planned_reward"),
        patch,
        save_offer_template,
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

async fn mark_dispatcher_job_terminal(
    connection: &mut sqlx::pool::PoolConnection<Sqlite>,
    vtc_job_id: &str,
    status: &str,
    completed_at_utc: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE dispatcher_jobs
        SET status = ?2,
            completed_at_utc = ?3,
            updated_at_utc = ?3
        WHERE id = ?1
        "#,
    )
    .bind(vtc_job_id)
    .bind(status)
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
    } else if payload.job_cancelled {
        "job_cancelled"
    } else {
        "job_failed"
    };
    let revenue = if payload.job_delivered {
        payload.job_delivered_revenue.max(payload.job_income)
    } else {
        0
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
    .bind(revenue)
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
    use super::{create_pool, resolve_prepare_save_mapping};
    use crate::features::ets2save::errors::AppErrorCode;
    use crate::features::ets2save::models::VtcDispatcherJob;
    use crate::features::ets2save::snapshot::{SaveSnapshotDepotDto, SaveSnapshotDto};

    fn fixture_dispatcher_job() -> VtcDispatcherJob {
        VtcDispatcherJob {
            vtc_job_id: "vtc-1".to_string(),
            source_type: "generated".to_string(),
            company_id: "north-axis-logistics".to_string(),
            company_name: "North Axis Logistics".to_string(),
            payment_tier: Some("standard".to_string()),
            job_type: "quick_job".to_string(),
            cargo_type: "trucks".to_string(),
            cargo_mass_kg: 12000.0,
            urgency_level: "normal".to_string(),
            difficulty_level: "normal".to_string(),
            equipment_type_required: "quick_job".to_string(),
            trailer_type_required: None,
            origin_city: "Berlin".to_string(),
            origin_country: "DE".to_string(),
            destination_city: "Hamburg".to_string(),
            destination_country: "DE".to_string(),
            route_distance_km: 520.0,
            estimated_duration_minutes: 360,
            base_rate_per_km: 1.12,
            calculated_rate_per_km: 1.18,
            total_reward: 758,
            profile_reference: None,
            quicksave_reference: None,
            save_reference: None,
            save_session_id: Some("session-1".to_string()),
            route_reference: None,
            dispatcher_status: Some("assigned_to_save".to_string()),
            last_error_code: None,
            last_error_message: None,
        }
    }

    fn fixture_snapshot(depots: Vec<SaveSnapshotDepotDto>) -> SaveSnapshotDto {
        SaveSnapshotDto {
            save_session_id: "session-1".to_string(),
            profile_reference: None,
            save_reference: None,
            quicksave_reference: None,
            captured_at_utc: "now".to_string(),
            checksum: "checksum".to_string(),
            visited_city_tokens: vec!["berlin".to_string()],
            transported_cargo_tokens: vec!["trucks".to_string()],
            selected_job_pointer: None,
            job_info_pointer: None,
            depots,
        }
    }

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

    #[test]
    fn host_selection_skips_offerless_depots() {
        tauri::async_runtime::block_on(async {
            let db_path =
                std::env::temp_dir().join(format!("ets_job_links_{}.sqlite", uuid::Uuid::new_v4()));
            let pool = create_pool(&db_path).await.unwrap();
            let mut connection = pool.acquire().await.unwrap();
            let snapshot = fixture_snapshot(vec![
                SaveSnapshotDepotDto {
                    company_token: "north_axis_logistics".to_string(),
                    city_token: "berlin".to_string(),
                    depot_key: "company.volatile.north_axis_logistics.berlin".to_string(),
                    discovered: true,
                    job_offer_count: 0,
                },
                SaveSnapshotDepotDto {
                    company_token: "tradeaux".to_string(),
                    city_token: "berlin".to_string(),
                    depot_key: "company.volatile.tradeaux.berlin".to_string(),
                    discovered: true,
                    job_offer_count: 2,
                },
            ]);

            let resolved =
                resolve_prepare_save_mapping(&mut connection, &fixture_dispatcher_job(), &snapshot)
                    .await
                    .unwrap();
            assert_eq!(resolved.resolved_src_company_token, "tradeaux");
            assert_eq!(resolved.resolution_mode, "fallback_host");
        });
    }

    #[test]
    fn host_selection_returns_actionable_error_when_city_has_no_offers() {
        tauri::async_runtime::block_on(async {
            let db_path =
                std::env::temp_dir().join(format!("ets_job_links_{}.sqlite", uuid::Uuid::new_v4()));
            let pool = create_pool(&db_path).await.unwrap();
            let mut connection = pool.acquire().await.unwrap();
            let snapshot = fixture_snapshot(vec![
                SaveSnapshotDepotDto {
                    company_token: "north_axis_logistics".to_string(),
                    city_token: "berlin".to_string(),
                    depot_key: "company.volatile.north_axis_logistics.berlin".to_string(),
                    discovered: true,
                    job_offer_count: 0,
                },
                SaveSnapshotDepotDto {
                    company_token: "tradeaux".to_string(),
                    city_token: "berlin".to_string(),
                    depot_key: "company.volatile.tradeaux.berlin".to_string(),
                    discovered: true,
                    job_offer_count: 0,
                },
            ]);

            let error =
                resolve_prepare_save_mapping(&mut connection, &fixture_dispatcher_job(), &snapshot)
                    .await
                    .unwrap_err();
            assert_eq!(error.code, AppErrorCode::CompanyHasNoJobOffersInCity);
            assert!(error.message.contains("city=berlin"));
            assert!(error.message.contains("offerless_count=2"));
        });
    }
}
