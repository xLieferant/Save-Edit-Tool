use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Emitter};

use crate::features::ets2save::errors::{AppError, AppErrorCode};
use crate::features::ets2save::parser::{SaveTemplateScan, scan_save_templates};
use crate::features::ets2save::sii_codec::decode_sii_lines;

const EVT_SAVE_SNAPSHOT_PROGRESS: &str = "vtc://save_snapshot/progress";
const EVT_SAVE_SNAPSHOT_DONE: &str = "vtc://save_snapshot/done";
const EVT_SAVE_SNAPSHOT_ERROR: &str = "vtc://save_snapshot/error";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSnapshotDepotDto {
    pub company_token: String,
    pub city_token: String,
    pub depot_key: String,
    pub discovered: bool,
    pub job_offer_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSnapshotDto {
    pub save_session_id: String,
    pub profile_reference: Option<String>,
    pub save_reference: Option<String>,
    pub quicksave_reference: Option<String>,
    pub captured_at_utc: String,
    pub checksum: String,
    pub visited_city_tokens: Vec<String>,
    pub transported_cargo_tokens: Vec<String>,
    pub selected_job_pointer: Option<String>,
    pub job_info_pointer: Option<String>,
    pub depots: Vec<SaveSnapshotDepotDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSnapshotDiagnosticsDto {
    pub snapshot_db_path: String,
    pub active_save_session_id: String,
    pub depot_count: i64,
    pub depot_with_offers_count: i64,
    pub offerless_depot_count: i64,
    pub visited_city_count: i64,
    pub cargo_count: i64,
    pub last_snapshot_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SaveSnapshotInput {
    pub save_session_id: String,
    pub profile_reference: Option<String>,
    pub save_reference: Option<String>,
    pub quicksave_reference: Option<String>,
}

pub async fn snapshot_refresh(
    app: Option<&AppHandle>,
    pool: &SqlitePool,
    input: SaveSnapshotInput,
) -> Result<SaveSnapshotDto, AppError> {
    emit_progress(app, &input.save_session_id, "loading_save");
    let save_reference = input
        .save_reference
        .clone()
        .ok_or_else(|| AppError::new(AppErrorCode::SaveNotFound, "save_reference missing"))?;
    let game_sii_path = PathBuf::from(&save_reference).join("game.sii");
    let lines = decode_sii_lines(&game_sii_path)?;
    let scan = scan_save_templates(&lines);
    let checksum = checksum_from_lines(&lines);

    emit_progress(app, &input.save_session_id, "persisting_snapshot");
    persist_snapshot(pool, &input, &scan, &checksum).await?;
    let dto = snapshot_get_by_session(pool, &input.save_session_id)
        .await?
        .ok_or_else(|| {
            AppError::new(
                AppErrorCode::InvalidToken,
                format!(
                    "snapshot missing after persist for {}",
                    input.save_session_id
                ),
            )
        })?;
    crate::dev_log!(
        "[ets2save] snapshot persisted session={} depots={} visited_cities={} cargo_tokens={} checksum={}",
        dto.save_session_id,
        dto.depots.len(),
        dto.visited_city_tokens.len(),
        dto.transported_cargo_tokens.len(),
        dto.checksum
    );
    emit_done(app, &dto);
    Ok(dto)
}

pub async fn snapshot_get_by_session(
    pool: &SqlitePool,
    save_session_id: &str,
) -> Result<Option<SaveSnapshotDto>, AppError> {
    let snapshot_row = sqlx::query(
        r#"
        SELECT save_session_id, profile_reference, save_reference, quicksave_reference, captured_at_utc, checksum
        FROM ets_save_snapshot
        WHERE save_session_id = ?1
        "#,
    )
    .bind(save_session_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = snapshot_row else {
        return Ok(None);
    };

    let depots = sqlx::query(
        r#"
        SELECT company_token, city_token, depot_key, discovered, job_offer_count
        FROM ets_save_depots
        WHERE save_session_id = ?1
        ORDER BY city_token, company_token
        "#,
    )
    .bind(save_session_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| SaveSnapshotDepotDto {
        company_token: row.get("company_token"),
        city_token: row.get("city_token"),
        depot_key: row.get("depot_key"),
        discovered: row.get::<i64, _>("discovered") != 0,
        job_offer_count: row.get("job_offer_count"),
    })
    .collect::<Vec<_>>();

    let visited_city_tokens = sqlx::query(
        "SELECT city_token FROM ets_save_visited_cities WHERE save_session_id = ?1 ORDER BY city_token",
    )
    .bind(save_session_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| row.get::<String, _>("city_token"))
    .collect::<Vec<_>>();

    let transported_cargo_tokens = sqlx::query(
        "SELECT cargo_token FROM ets_save_transport_cargo WHERE save_session_id = ?1 ORDER BY cargo_token",
    )
    .bind(save_session_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| row.get::<String, _>("cargo_token"))
    .collect::<Vec<_>>();

    let selected_job_pointer = sqlx::query_scalar::<_, Option<String>>(
        "SELECT value FROM ets_save_snapshot_meta WHERE save_session_id = ?1 AND key = 'selected_job_pointer'",
    )
    .bind(save_session_id)
    .fetch_optional(pool)
    .await?
    .flatten();

    let job_info_pointer = sqlx::query_scalar::<_, Option<String>>(
        "SELECT value FROM ets_save_snapshot_meta WHERE save_session_id = ?1 AND key = 'job_info_pointer'",
    )
    .bind(save_session_id)
    .fetch_optional(pool)
    .await?
    .flatten();

    Ok(Some(SaveSnapshotDto {
        save_session_id: row.get("save_session_id"),
        profile_reference: row.try_get("profile_reference").ok(),
        save_reference: row.try_get("save_reference").ok(),
        quicksave_reference: row.try_get("quicksave_reference").ok(),
        captured_at_utc: row.get("captured_at_utc"),
        checksum: row.get("checksum"),
        visited_city_tokens,
        transported_cargo_tokens,
        selected_job_pointer,
        job_info_pointer,
        depots,
    }))
}

pub async fn snapshot_list_depots(
    pool: &SqlitePool,
    save_session_id: &str,
    city_token: Option<&str>,
) -> Result<Vec<SaveSnapshotDepotDto>, AppError> {
    let rows = if let Some(city_token) = city_token {
        sqlx::query(
            r#"
            SELECT company_token, city_token, depot_key, discovered, job_offer_count
            FROM ets_save_depots
            WHERE save_session_id = ?1 AND city_token = ?2
            ORDER BY company_token
            "#,
        )
        .bind(save_session_id)
        .bind(city_token)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT company_token, city_token, depot_key, discovered, job_offer_count
            FROM ets_save_depots
            WHERE save_session_id = ?1
            ORDER BY city_token, company_token
            "#,
        )
        .bind(save_session_id)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(|row| SaveSnapshotDepotDto {
            company_token: row.get("company_token"),
            city_token: row.get("city_token"),
            depot_key: row.get("depot_key"),
            discovered: row.get::<i64, _>("discovered") != 0,
            job_offer_count: row.get("job_offer_count"),
        })
        .collect())
}

pub async fn snapshot_diagnostics_by_session(
    pool: &SqlitePool,
    save_session_id: &str,
) -> Result<Option<SaveSnapshotDiagnosticsDto>, AppError> {
    let db_path = resolve_sqlite_main_path(pool).await.unwrap_or_default();
    let last_snapshot_at = sqlx::query_scalar::<_, Option<String>>(
        "SELECT captured_at_utc FROM ets_save_snapshot WHERE save_session_id = ?1",
    )
    .bind(save_session_id)
    .fetch_optional(pool)
    .await?
    .flatten();

    if last_snapshot_at.is_none() {
        return Ok(None);
    }

    let depot_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ets_save_depots WHERE save_session_id = ?1",
    )
    .bind(save_session_id)
    .fetch_one(pool)
    .await?;

    let visited_city_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ets_save_visited_cities WHERE save_session_id = ?1",
    )
    .bind(save_session_id)
    .fetch_one(pool)
    .await?;

    let cargo_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ets_save_transport_cargo WHERE save_session_id = ?1",
    )
    .bind(save_session_id)
    .fetch_one(pool)
    .await?;

    let depot_with_offers_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ets_save_depots WHERE save_session_id = ?1 AND job_offer_count > 0",
    )
    .bind(save_session_id)
    .fetch_one(pool)
    .await?;

    Ok(Some(SaveSnapshotDiagnosticsDto {
        snapshot_db_path: db_path,
        active_save_session_id: save_session_id.to_string(),
        depot_count,
        depot_with_offers_count,
        offerless_depot_count: depot_count.saturating_sub(depot_with_offers_count),
        visited_city_count,
        cargo_count,
        last_snapshot_at,
    }))
}

pub fn depots_by_city(snapshot: &SaveSnapshotDto) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for depot in &snapshot.depots {
        map.entry(depot.city_token.clone())
            .or_default()
            .push(depot.company_token.clone());
    }
    for list in map.values_mut() {
        list.sort();
        list.dedup();
    }
    map
}

fn checksum_from_lines(lines: &[String]) -> String {
    let joined = lines.join("\n");
    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    format!("{:x}", hasher.finalize())
}

async fn resolve_sqlite_main_path(pool: &SqlitePool) -> Result<String, AppError> {
    let rows = sqlx::query("PRAGMA database_list").fetch_all(pool).await?;
    for row in rows {
        let name: String = row.get("name");
        if name == "main" {
            let path: String = row.get("file");
            return Ok(path);
        }
    }
    Ok(String::new())
}

async fn persist_snapshot(
    pool: &SqlitePool,
    input: &SaveSnapshotInput,
    scan: &SaveTemplateScan,
    checksum: &str,
) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO ets_save_snapshot (
            save_session_id, profile_reference, save_reference, quicksave_reference, captured_at_utc, checksum
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(save_session_id) DO UPDATE SET
            profile_reference = excluded.profile_reference,
            save_reference = excluded.save_reference,
            quicksave_reference = excluded.quicksave_reference,
            captured_at_utc = excluded.captured_at_utc,
            checksum = excluded.checksum
        "#,
    )
    .bind(&input.save_session_id)
    .bind(input.profile_reference.as_deref())
    .bind(input.save_reference.as_deref())
    .bind(input.quicksave_reference.as_deref())
    .bind(&now)
    .bind(checksum)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM ets_save_depots WHERE save_session_id = ?1")
        .bind(&input.save_session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM ets_save_visited_cities WHERE save_session_id = ?1")
        .bind(&input.save_session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM ets_save_transport_cargo WHERE save_session_id = ?1")
        .bind(&input.save_session_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM ets_save_snapshot_meta WHERE save_session_id = ?1")
        .bind(&input.save_session_id)
        .execute(&mut *tx)
        .await?;

    for depot in &scan.depots {
        sqlx::query(
            r#"
            INSERT INTO ets_save_depots (
                save_session_id, company_token, city_token, depot_key, discovered, job_offer_count
            )
            VALUES (?1, ?2, ?3, ?4, 1, ?5)
            "#,
        )
        .bind(&input.save_session_id)
        .bind(&depot.company_token)
        .bind(&depot.city_token)
        .bind(&depot.unit_token)
        .bind(depot.job_offer_count as i64)
        .execute(&mut *tx)
        .await?;
    }

    for city in &scan.visited_cities {
        sqlx::query(
            "INSERT INTO ets_save_visited_cities (save_session_id, city_token) VALUES (?1, ?2)",
        )
        .bind(&input.save_session_id)
        .bind(city)
        .execute(&mut *tx)
        .await?;
    }

    for cargo in &scan.transported_cargo_tokens {
        sqlx::query(
            "INSERT INTO ets_save_transport_cargo (save_session_id, cargo_token) VALUES (?1, ?2)",
        )
        .bind(&input.save_session_id)
        .bind(cargo)
        .execute(&mut *tx)
        .await?;
    }

    if let Some(pointer) = scan.selected_job_pointer.as_deref() {
        sqlx::query(
            "INSERT INTO ets_save_snapshot_meta (save_session_id, key, value) VALUES (?1, 'selected_job_pointer', ?2)",
        )
        .bind(&input.save_session_id)
        .bind(pointer)
        .execute(&mut *tx)
        .await?;
    }
    if let Some(pointer) = scan
        .job_info_units
        .first()
        .map(|item| item.pointer.as_str())
    {
        sqlx::query(
            "INSERT INTO ets_save_snapshot_meta (save_session_id, key, value) VALUES (?1, 'job_info_pointer', ?2)",
        )
        .bind(&input.save_session_id)
        .bind(pointer)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

fn emit_progress(app: Option<&AppHandle>, save_session_id: &str, stage: &str) {
    let Some(app) = app else {
        return;
    };
    let _ = app.emit(
        EVT_SAVE_SNAPSHOT_PROGRESS,
        serde_json::json!({ "saveSessionId": save_session_id, "stage": stage }),
    );
}

fn emit_done(app: Option<&AppHandle>, snapshot: &SaveSnapshotDto) {
    let Some(app) = app else {
        return;
    };
    let _ = app.emit(EVT_SAVE_SNAPSHOT_DONE, snapshot);
}

pub fn emit_error(app: Option<&AppHandle>, save_session_id: &str, error: &str) {
    let Some(app) = app else {
        return;
    };
    let _ = app.emit(
        EVT_SAVE_SNAPSHOT_ERROR,
        serde_json::json!({ "saveSessionId": save_session_id, "error": error }),
    );
}
