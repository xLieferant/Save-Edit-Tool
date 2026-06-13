use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::features::ets2save::dispatcher as save_dispatcher;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::extract::extract_profile_name;
use crate::shared::extract_save_name::extract_save_name;
use crate::shared::sqlite_schema::{create_indexes, ensure_columns};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsFilters {
    pub from: Option<String>,
    pub to: Option<String>,
    pub profile: Option<String>,
    pub game: Option<String>,
    pub status: Option<String>,
    pub cargo: Option<String>,
    pub source_city: Option<String>,
    pub destination_city: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSourceCounts {
    pub local_tracking: i64,
    pub save_import: i64,
    pub partial_save_data: i64,
    pub unknown: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummary {
    pub total_jobs: i64,
    pub total_revenue: Option<i64>,
    pub total_costs: Option<i64>,
    pub total_profit: Option<i64>,
    pub total_distance_km: Option<f64>,
    pub average_profit_per_job: Option<f64>,
    pub average_profit_per_km: Option<f64>,
    pub damaged_deliveries: i64,
    pub on_time_deliveries: Option<i64>,
    pub late_deliveries: Option<i64>,
    pub source_counts: AnalyticsSourceCounts,
    pub partial_data: bool,
    pub has_local_tracking: bool,
    pub has_save_import_data: bool,
    pub known_profit_rows: i64,
    pub known_cost_rows: i64,
    pub known_distance_rows: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsChartPoint {
    pub label: String,
    pub value: f64,
    pub value_secondary: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsCharts {
    pub revenue_over_time: Vec<AnalyticsChartPoint>,
    pub profit_over_time: Vec<AnalyticsChartPoint>,
    pub jobs_per_day: Vec<AnalyticsChartPoint>,
    pub top_cargo: Vec<AnalyticsChartPoint>,
    pub top_routes: Vec<AnalyticsChartPoint>,
    pub damage_cost_analysis: Vec<AnalyticsChartPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsFilterOptions {
    pub profiles: Vec<String>,
    pub cargos: Vec<String>,
    pub source_cities: Vec<String>,
    pub destination_cities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsJobHistoryItem {
    pub id: i64,
    pub game: Option<String>,
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub save_path: Option<String>,
    pub source: String,
    pub source_save_name: Option<String>,
    pub detected_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub job_uid: String,
    pub status: String,
    pub cargo_name: Option<String>,
    pub source_city: Option<String>,
    pub destination_city: Option<String>,
    pub source_company: Option<String>,
    pub destination_company: Option<String>,
    pub distance_km: Option<f64>,
    pub revenue: Option<i64>,
    pub costs: Option<i64>,
    pub penalties: Option<i64>,
    pub damage_percent: Option<f64>,
    pub profit: Option<i64>,
    pub xp: Option<i64>,
    pub level_after: Option<i64>,
    pub truck_name: Option<String>,
    pub trailer_name: Option<String>,
    pub driven_with_truck: Option<bool>,
    pub data_origin_note: Option<String>,
    pub raw_data_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsJobHistoryResponse {
    pub items: Vec<AnalyticsJobHistoryItem>,
    pub charts: AnalyticsCharts,
    pub filter_options: AnalyticsFilterOptions,
    pub partial_data: bool,
    pub total_filtered_jobs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsScanResult {
    pub profile_path: String,
    pub profile_id: String,
    pub profile_name: String,
    pub scanned_saves: i64,
    pub detected_jobs: i64,
    pub inserted_jobs: i64,
    pub updated_jobs: i64,
    pub sources_used: Vec<String>,
    pub note: String,
}

#[derive(Debug, Clone)]
struct ScannedJobRecord {
    game: Option<String>,
    profile_id: String,
    profile_name: String,
    save_path: String,
    source: String,
    source_save_name: String,
    detected_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    job_uid: String,
    status: String,
    cargo_name: Option<String>,
    source_city: Option<String>,
    destination_city: Option<String>,
    source_company: Option<String>,
    destination_company: Option<String>,
    distance_km: Option<f64>,
    revenue: Option<i64>,
    costs: Option<i64>,
    penalties: Option<i64>,
    damage_percent: Option<f64>,
    profit: Option<i64>,
    xp: Option<i64>,
    level_after: Option<i64>,
    truck_name: Option<String>,
    trailer_name: Option<String>,
    driven_with_truck: Option<bool>,
    data_origin_note: Option<String>,
    raw_data_json: Option<String>,
}

pub fn ensure_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS career_job_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            game TEXT,
            profile_id TEXT,
            profile_name TEXT,
            save_path TEXT,
            source TEXT NOT NULL DEFAULT 'unknown',
            source_save_name TEXT,
            detected_at TEXT NOT NULL,
            started_at TEXT,
            completed_at TEXT,
            job_uid TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL DEFAULT 'unknown',
            cargo_name TEXT,
            source_city TEXT,
            destination_city TEXT,
            source_company TEXT,
            destination_company TEXT,
            distance_km REAL,
            revenue INTEGER,
            costs INTEGER,
            penalties INTEGER,
            damage_percent REAL,
            profit INTEGER,
            xp INTEGER,
            level_after INTEGER,
            truck_name TEXT,
            trailer_name TEXT,
            driven_with_truck INTEGER,
            data_origin_note TEXT,
            raw_data_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|error| error.to_string())?;

    ensure_columns(
        conn,
        "career_job_history",
        &[
            ("game", "TEXT"),
            ("profile_id", "TEXT"),
            ("profile_name", "TEXT"),
            ("save_path", "TEXT"),
            ("source", "TEXT NOT NULL DEFAULT 'unknown'"),
            ("source_save_name", "TEXT"),
            ("detected_at", "TEXT NOT NULL DEFAULT ''"),
            ("started_at", "TEXT"),
            ("completed_at", "TEXT"),
            ("job_uid", "TEXT NOT NULL DEFAULT ''"),
            ("status", "TEXT NOT NULL DEFAULT 'unknown'"),
            ("cargo_name", "TEXT"),
            ("source_city", "TEXT"),
            ("destination_city", "TEXT"),
            ("source_company", "TEXT"),
            ("destination_company", "TEXT"),
            ("distance_km", "REAL"),
            ("revenue", "INTEGER"),
            ("costs", "INTEGER"),
            ("penalties", "INTEGER"),
            ("damage_percent", "REAL"),
            ("profit", "INTEGER"),
            ("xp", "INTEGER"),
            ("level_after", "INTEGER"),
            ("truck_name", "TEXT"),
            ("trailer_name", "TEXT"),
            ("driven_with_truck", "INTEGER"),
            ("data_origin_note", "TEXT"),
            ("raw_data_json", "TEXT"),
            ("created_at", "TEXT NOT NULL DEFAULT ''"),
            ("updated_at", "TEXT NOT NULL DEFAULT ''"),
        ],
    )?;
    create_indexes(
        conn,
        &[
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_career_job_history_uid ON career_job_history(job_uid)",
            "CREATE INDEX IF NOT EXISTS idx_career_job_history_profile ON career_job_history(profile_id, detected_at DESC)",
            "CREATE INDEX IF NOT EXISTS idx_career_job_history_status ON career_job_history(status, detected_at DESC)",
            "CREATE INDEX IF NOT EXISTS idx_career_job_history_source ON career_job_history(source, detected_at DESC)",
        ],
    )?;

    backfill_from_job_log(conn)?;
    Ok(())
}

fn backfill_from_job_log(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        INSERT INTO career_job_history (
            game,
            profile_id,
            profile_name,
            save_path,
            source,
            source_save_name,
            detected_at,
            started_at,
            completed_at,
            job_uid,
            status,
            cargo_name,
            source_city,
            destination_city,
            source_company,
            destination_company,
            distance_km,
            revenue,
            costs,
            penalties,
            damage_percent,
            profit,
            xp,
            level_after,
            truck_name,
            trailer_name,
            driven_with_truck,
            data_origin_note,
            raw_data_json,
            created_at,
            updated_at
        )
        SELECT
            NULL,
            NULL,
            NULL,
            NULL,
            'local_tracking',
            NULL,
            COALESCE(last_seen_at_utc, started_at_utc, CURRENT_TIMESTAMP),
            started_at_utc,
            ended_at_utc,
            job_id,
            CASE
                WHEN status IN ('active', 'completed', 'failed', 'cancelled') THEN status
                ELSE 'unknown'
            END,
            cargo,
            origin_city,
            destination_city,
            source_company,
            destination_company,
            CASE WHEN planned_distance_km > 0 THEN planned_distance_km ELSE NULL END,
            CASE WHEN income <> 0 THEN income ELSE NULL END,
            NULL,
            NULL,
            CASE WHEN cargo_damage > 0 THEN cargo_damage ELSE NULL END,
            NULL,
            NULL,
            NULL,
            NULL,
            NULL,
            NULL,
            'Backfilled from career_job_log.',
            NULL,
            COALESCE(started_at_utc, last_seen_at_utc, CURRENT_TIMESTAMP),
            COALESCE(last_seen_at_utc, started_at_utc, CURRENT_TIMESTAMP)
        FROM career_job_log
        ON CONFLICT(job_uid) DO UPDATE SET
            detected_at = excluded.detected_at,
            started_at = COALESCE(excluded.started_at, career_job_history.started_at),
            completed_at = COALESCE(excluded.completed_at, career_job_history.completed_at),
            status = CASE
                WHEN career_job_history.status = 'completed' THEN career_job_history.status
                ELSE excluded.status
            END,
            cargo_name = COALESCE(excluded.cargo_name, career_job_history.cargo_name),
            source_city = COALESCE(excluded.source_city, career_job_history.source_city),
            destination_city = COALESCE(excluded.destination_city, career_job_history.destination_city),
            source_company = COALESCE(excluded.source_company, career_job_history.source_company),
            destination_company = COALESCE(excluded.destination_company, career_job_history.destination_company),
            distance_km = COALESCE(excluded.distance_km, career_job_history.distance_km),
            revenue = COALESCE(excluded.revenue, career_job_history.revenue),
            damage_percent = COALESCE(excluded.damage_percent, career_job_history.damage_percent),
            updated_at = excluded.updated_at
        ;
        "#,
    )
    .map_err(|error| error.to_string())?;

    Ok(())
}

pub fn load_summary(
    conn: &Connection,
    filters: Option<AnalyticsFilters>,
) -> Result<AnalyticsSummary, String> {
    ensure_tables(conn)?;
    let items = load_filtered_items(conn, filters.as_ref())?;
    Ok(build_summary(&items))
}

pub fn load_history(
    conn: &Connection,
    filters: Option<AnalyticsFilters>,
) -> Result<AnalyticsJobHistoryResponse, String> {
    ensure_tables(conn)?;
    let all_items = load_filtered_items(conn, None)?;
    let filtered_items = apply_filters(&all_items, filters.as_ref());

    Ok(AnalyticsJobHistoryResponse {
        charts: build_charts(&filtered_items),
        filter_options: build_filter_options(&all_items),
        partial_data: build_summary(&filtered_items).partial_data,
        total_filtered_jobs: filtered_items.len() as i64,
        items: filtered_items,
    })
}

pub fn scan_profile_job_history(
    conn: &Connection,
    profile_path: &str,
    selected_game: Option<&str>,
) -> Result<AnalyticsScanResult, String> {
    ensure_tables(conn)?;

    let normalized_profile = profile_path.trim().replace('\\', "/");
    let profile_dir = PathBuf::from(&normalized_profile);
    if !profile_dir.exists() {
        return Err(format!("Profile path not found: {}", normalized_profile));
    }

    let profile_id = profile_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown-profile")
        .to_string();
    let profile_name = resolve_profile_name(&profile_dir).unwrap_or_else(|| profile_id.clone());
    let game = infer_game(&normalized_profile, selected_game);

    let save_root = profile_dir.join("save");
    if !save_root.is_dir() {
        return Err(format!(
            "Save directory not found for profile: {}",
            normalized_profile
        ));
    }

    let mut scanned_saves = 0_i64;
    let mut detected_jobs = 0_i64;
    let mut inserted_jobs = 0_i64;
    let mut updated_jobs = 0_i64;
    let mut sources_used = BTreeSet::new();

    let entries = fs::read_dir(&save_root).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let save_dir = entry.path();
        if !save_dir.is_dir() {
            continue;
        }

        let game_sii_path = save_dir.join("game.sii");
        if !game_sii_path.exists() {
            continue;
        }

        scanned_saves += 1;
        let save_name = resolve_save_name(&save_dir).unwrap_or_else(|| {
            save_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("save")
                .to_string()
        });
        let content = match decrypt_if_needed(&game_sii_path) {
            Ok(value) => value,
            Err(error) => {
                crate::dev_log!(
                    "[analytics] save import skipped unreadable game.sii path={} error={}",
                    game_sii_path.display(),
                    error
                );
                continue;
            }
        };

        let inspection = match save_dispatcher::inspect_quicksave(&content) {
            Ok(value) => value,
            Err(error) => {
                crate::dev_log!(
                    "[analytics] save import inspect failed path={} error={}",
                    game_sii_path.display(),
                    error
                );
                continue;
            }
        };

        let player_xp = extract_integer_field(&content, "experience_points");
        let detected_at = fs::metadata(&game_sii_path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|value| DateTime::<Utc>::from_timestamp(value.as_secs() as i64, 0))
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| Utc::now().to_rfc3339());

        if let Some(selected) = inspection.selected_job.as_ref() {
            let scanned = build_selected_job_record(
                &game,
                &profile_id,
                &profile_name,
                &save_dir,
                &save_name,
                &detected_at,
                player_xp,
                selected,
            );
            let was_existing = get_item_by_uid(conn, &scanned.job_uid)?.is_some();
            upsert_scanned_job(conn, &scanned)?;
            detected_jobs += 1;
            if was_existing {
                updated_jobs += 1;
            } else {
                inserted_jobs += 1;
            }
            sources_used.insert(scanned.source.clone());
        }

        if let Some(active) = inspection.active_job.as_ref() {
            let scanned = build_active_job_record(
                &game,
                &profile_id,
                &profile_name,
                &save_dir,
                &save_name,
                &detected_at,
                player_xp,
                active,
            );
            let was_existing = get_item_by_uid(conn, &scanned.job_uid)?.is_some();
            upsert_scanned_job(conn, &scanned)?;
            detected_jobs += 1;
            if was_existing {
                updated_jobs += 1;
            } else {
                inserted_jobs += 1;
            }
            sources_used.insert(scanned.source.clone());
        }
    }

    Ok(AnalyticsScanResult {
        profile_path: normalized_profile,
        profile_id,
        profile_name,
        scanned_saves,
        detected_jobs,
        inserted_jobs,
        updated_jobs,
        sources_used: sources_used.into_iter().collect(),
        note: "Imported currently readable save-linked job data. Historical completeness is limited by ETS2/ATS save contents.".to_string(),
    })
}

pub fn export_csv(
    app: &AppHandle,
    conn: &Connection,
    filters: Option<AnalyticsFilters>,
) -> Result<Option<String>, String> {
    ensure_tables(conn)?;
    let items = load_filtered_items(conn, filters.as_ref())?;
    let date_stamp = Utc::now().format("%Y-%m-%d").to_string();
    let default_file_name = format!("simnexus_analytics_{}.csv", date_stamp);
    let file_path = app
        .dialog()
        .file()
        .add_filter("CSV file", &["csv"])
        .set_title("Export analytics CSV")
        .set_file_name(&default_file_name)
        .blocking_save_file();

    let Some(file_path) = file_path else {
        return Ok(None);
    };

    let path = file_path
        .into_path()
        .map_err(|_| "The selected export path could not be resolved.".to_string())?;

    let csv = build_csv(&items);
    fs::write(&path, csv.as_bytes()).map_err(|error| {
        format!(
            "The analytics CSV could not be written to {}: {}",
            path.display(),
            error
        )
    })?;

    Ok(Some(path.display().to_string()))
}

fn load_filtered_items(
    conn: &Connection,
    filters: Option<&AnalyticsFilters>,
) -> Result<Vec<AnalyticsJobHistoryItem>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                game,
                profile_id,
                profile_name,
                save_path,
                source,
                source_save_name,
                detected_at,
                started_at,
                completed_at,
                job_uid,
                status,
                cargo_name,
                source_city,
                destination_city,
                source_company,
                destination_company,
                distance_km,
                revenue,
                costs,
                penalties,
                damage_percent,
                profit,
                xp,
                level_after,
                truck_name,
                trailer_name,
                driven_with_truck,
                data_origin_note,
                raw_data_json,
                created_at,
                updated_at
            FROM career_job_history
            ORDER BY COALESCE(started_at, detected_at) DESC, id DESC
            "#,
        )
        .map_err(|error| error.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(AnalyticsJobHistoryItem {
                id: row.get(0)?,
                game: row.get(1)?,
                profile_id: row.get(2)?,
                profile_name: row.get(3)?,
                save_path: row.get(4)?,
                source: row.get(5)?,
                source_save_name: row.get(6)?,
                detected_at: row.get(7)?,
                started_at: row.get(8)?,
                completed_at: row.get(9)?,
                job_uid: row.get(10)?,
                status: row.get(11)?,
                cargo_name: row.get(12)?,
                source_city: row.get(13)?,
                destination_city: row.get(14)?,
                source_company: row.get(15)?,
                destination_company: row.get(16)?,
                distance_km: row.get(17)?,
                revenue: row.get(18)?,
                costs: row.get(19)?,
                penalties: row.get(20)?,
                damage_percent: row.get(21)?,
                profit: row.get(22)?,
                xp: row.get(23)?,
                level_after: row.get(24)?,
                truck_name: row.get(25)?,
                trailer_name: row.get(26)?,
                driven_with_truck: row.get::<_, Option<i64>>(27)?.map(|value| value != 0),
                data_origin_note: row.get(28)?,
                raw_data_json: row.get(29)?,
                created_at: row.get(30)?,
                updated_at: row.get(31)?,
            })
        })
        .map_err(|error| error.to_string())?;

    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(apply_filters(&items, filters))
}

fn apply_filters(
    items: &[AnalyticsJobHistoryItem],
    filters: Option<&AnalyticsFilters>,
) -> Vec<AnalyticsJobHistoryItem> {
    let Some(filters) = filters else {
        return items.to_vec();
    };

    let from = filters
        .from
        .as_deref()
        .and_then(|value| NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok());
    let to = filters
        .to
        .as_deref()
        .and_then(|value| NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok());
    let profile = normalized_filter(filters.profile.as_deref());
    let game = normalized_filter(filters.game.as_deref());
    let status = normalized_filter(filters.status.as_deref());
    let cargo = normalized_filter(filters.cargo.as_deref());
    let source_city = normalized_filter(filters.source_city.as_deref());
    let destination_city = normalized_filter(filters.destination_city.as_deref());

    items
        .iter()
        .filter(|item| {
            if let Some(from) = from {
                let item_date = item_primary_date(item);
                if item_date < from {
                    return false;
                }
            }
            if let Some(to) = to {
                let item_date = item_primary_date(item);
                if item_date > to {
                    return false;
                }
            }

            if let Some(profile) = profile.as_deref() {
                let profile_match = item
                    .profile_name
                    .as_deref()
                    .map(normalize_for_match)
                    .map(|value| value == profile)
                    .unwrap_or(false)
                    || item
                        .profile_id
                        .as_deref()
                        .map(normalize_for_match)
                        .map(|value| value == profile)
                        .unwrap_or(false);
                if !profile_match {
                    return false;
                }
            }

            if let Some(game) = game.as_deref() {
                let item_game = item
                    .game
                    .as_deref()
                    .map(normalize_for_match)
                    .unwrap_or_default();
                if item_game != game {
                    return false;
                }
            }

            if let Some(status) = status.as_deref() {
                if normalize_for_match(&item.status) != status {
                    return false;
                }
            }

            if let Some(cargo) = cargo.as_deref() {
                if !contains_text(item.cargo_name.as_deref(), cargo) {
                    return false;
                }
            }

            if let Some(source_city) = source_city.as_deref() {
                if !contains_text(item.source_city.as_deref(), source_city) {
                    return false;
                }
            }

            if let Some(destination_city) = destination_city.as_deref() {
                if !contains_text(item.destination_city.as_deref(), destination_city) {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect()
}

fn build_summary(items: &[AnalyticsJobHistoryItem]) -> AnalyticsSummary {
    let mut summary = AnalyticsSummary::default();
    summary.total_jobs = items.len() as i64;

    let mut total_revenue = 0_i64;
    let mut total_revenue_known = false;
    let mut total_costs = 0_i64;
    let mut total_costs_known = false;
    let mut total_profit = 0_i64;
    let mut total_profit_known = false;
    let mut total_distance = 0.0_f64;
    let mut total_distance_known = false;

    for item in items {
        match item.source.as_str() {
            "local_tracking" => summary.source_counts.local_tracking += 1,
            "save_import" => summary.source_counts.save_import += 1,
            "partial_save_data" => summary.source_counts.partial_save_data += 1,
            _ => summary.source_counts.unknown += 1,
        }

        if let Some(revenue) = item.revenue {
            total_revenue += revenue;
            total_revenue_known = true;
        }
        if let Some(costs) = item.costs {
            total_costs += costs;
            total_costs_known = true;
            summary.known_cost_rows += 1;
        }
        if let Some(profit) = item.profit {
            total_profit += profit;
            total_profit_known = true;
            summary.known_profit_rows += 1;
        }
        if let Some(distance) = item.distance_km {
            total_distance += distance;
            total_distance_known = true;
            summary.known_distance_rows += 1;
        }
        if item.damage_percent.unwrap_or(0.0) > 0.0 {
            summary.damaged_deliveries += 1;
        }
    }

    summary.total_revenue = total_revenue_known.then_some(total_revenue);
    summary.total_costs = total_costs_known.then_some(total_costs);
    summary.total_profit = total_profit_known.then_some(total_profit);
    summary.total_distance_km = total_distance_known.then_some(total_distance);
    summary.has_local_tracking = summary.source_counts.local_tracking > 0;
    summary.has_save_import_data =
        summary.source_counts.save_import > 0 || summary.source_counts.partial_save_data > 0;
    summary.partial_data = summary.source_counts.partial_save_data > 0
        || summary.known_profit_rows < summary.total_jobs
        || summary.known_cost_rows < summary.total_jobs
        || summary.known_distance_rows < summary.total_jobs
        || summary.on_time_deliveries.is_none()
        || summary.late_deliveries.is_none();

    if summary.known_profit_rows > 0 {
        summary.average_profit_per_job =
            Some(total_profit as f64 / summary.known_profit_rows as f64);
    }
    if summary.known_profit_rows > 0 && total_distance_known && total_distance > 0.0 {
        summary.average_profit_per_km = Some(total_profit as f64 / total_distance);
    }

    summary
}

fn build_charts(items: &[AnalyticsJobHistoryItem]) -> AnalyticsCharts {
    AnalyticsCharts {
        revenue_over_time: build_time_chart(items, |item| item.revenue.map(|value| value as f64)),
        profit_over_time: build_time_chart(items, |item| item.profit.map(|value| value as f64)),
        jobs_per_day: build_jobs_per_day(items),
        top_cargo: build_top_dimension(items, |item| item.cargo_name.clone(), 8),
        top_routes: build_top_dimension(
            items,
            |item| {
                Some(format!(
                    "{} -> {}",
                    safe_label(item.source_city.as_deref()),
                    safe_label(item.destination_city.as_deref())
                ))
            },
            8,
        ),
        damage_cost_analysis: build_damage_cost_chart(items),
    }
}

fn build_filter_options(items: &[AnalyticsJobHistoryItem]) -> AnalyticsFilterOptions {
    let mut profiles = BTreeSet::new();
    let mut cargos = BTreeSet::new();
    let mut source_cities = BTreeSet::new();
    let mut destination_cities = BTreeSet::new();

    for item in items {
        if let Some(value) = item
            .profile_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            profiles.insert(value.trim().to_string());
        }
        if let Some(value) = item
            .cargo_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            cargos.insert(value.trim().to_string());
        }
        if let Some(value) = item
            .source_city
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            source_cities.insert(value.trim().to_string());
        }
        if let Some(value) = item
            .destination_city
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            destination_cities.insert(value.trim().to_string());
        }
    }

    AnalyticsFilterOptions {
        profiles: profiles.into_iter().collect(),
        cargos: cargos.into_iter().collect(),
        source_cities: source_cities.into_iter().collect(),
        destination_cities: destination_cities.into_iter().collect(),
    }
}

fn build_time_chart<F>(items: &[AnalyticsJobHistoryItem], value_of: F) -> Vec<AnalyticsChartPoint>
where
    F: Fn(&AnalyticsJobHistoryItem) -> Option<f64>,
{
    let mut buckets: BTreeMap<String, f64> = BTreeMap::new();
    for item in items {
        let Some(value) = value_of(item) else {
            continue;
        };
        let key = item_primary_date(item).format("%Y-%m-%d").to_string();
        *buckets.entry(key).or_insert(0.0) += value;
    }

    buckets
        .into_iter()
        .rev()
        .take(10)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|(label, value)| AnalyticsChartPoint {
            label,
            value,
            value_secondary: None,
        })
        .collect()
}

fn build_jobs_per_day(items: &[AnalyticsJobHistoryItem]) -> Vec<AnalyticsChartPoint> {
    let mut buckets: BTreeMap<String, f64> = BTreeMap::new();
    for item in items {
        let key = item_primary_date(item).format("%Y-%m-%d").to_string();
        *buckets.entry(key).or_insert(0.0) += 1.0;
    }

    buckets
        .into_iter()
        .rev()
        .take(10)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|(label, value)| AnalyticsChartPoint {
            label,
            value,
            value_secondary: None,
        })
        .collect()
}

fn build_top_dimension<F>(
    items: &[AnalyticsJobHistoryItem],
    label_of: F,
    limit: usize,
) -> Vec<AnalyticsChartPoint>
where
    F: Fn(&AnalyticsJobHistoryItem) -> Option<String>,
{
    let mut counts: HashMap<String, f64> = HashMap::new();
    for item in items {
        let Some(label) = label_of(item) else {
            continue;
        };
        if label.trim().is_empty() {
            continue;
        }
        *counts.entry(label).or_insert(0.0) += 1.0;
    }

    let mut rows = counts.into_iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    rows.into_iter()
        .take(limit)
        .map(|(label, value)| AnalyticsChartPoint {
            label,
            value,
            value_secondary: None,
        })
        .collect()
}

fn build_damage_cost_chart(items: &[AnalyticsJobHistoryItem]) -> Vec<AnalyticsChartPoint> {
    let mut rows = items
        .iter()
        .filter_map(|item| {
            let damage = item.damage_percent.unwrap_or(0.0);
            let costs = item.costs.unwrap_or(0) + item.penalties.unwrap_or(0);
            if damage <= 0.0 && costs <= 0 {
                return None;
            }

            Some(AnalyticsChartPoint {
                label: format!(
                    "{} -> {}",
                    safe_label(item.source_city.as_deref()),
                    safe_label(item.destination_city.as_deref())
                ),
                value: damage,
                value_secondary: Some(costs as f64),
            })
        })
        .collect::<Vec<_>>();

    rows.sort_by(|left, right| {
        let left_score = left.value + left.value_secondary.unwrap_or(0.0);
        let right_score = right.value + right.value_secondary.unwrap_or(0.0);
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(8);
    rows
}

fn upsert_scanned_job(conn: &Connection, item: &ScannedJobRecord) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        r#"
        INSERT INTO career_job_history (
            game,
            profile_id,
            profile_name,
            save_path,
            source,
            source_save_name,
            detected_at,
            started_at,
            completed_at,
            job_uid,
            status,
            cargo_name,
            source_city,
            destination_city,
            source_company,
            destination_company,
            distance_km,
            revenue,
            costs,
            penalties,
            damage_percent,
            profit,
            xp,
            level_after,
            truck_name,
            trailer_name,
            driven_with_truck,
            data_origin_note,
            raw_data_json,
            created_at,
            updated_at
        )
        VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
            ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31
        )
        ON CONFLICT(job_uid) DO UPDATE SET
            game = COALESCE(excluded.game, career_job_history.game),
            profile_id = COALESCE(excluded.profile_id, career_job_history.profile_id),
            profile_name = COALESCE(excluded.profile_name, career_job_history.profile_name),
            save_path = COALESCE(excluded.save_path, career_job_history.save_path),
            source = excluded.source,
            source_save_name = COALESCE(excluded.source_save_name, career_job_history.source_save_name),
            detected_at = excluded.detected_at,
            started_at = COALESCE(excluded.started_at, career_job_history.started_at),
            completed_at = COALESCE(excluded.completed_at, career_job_history.completed_at),
            status = excluded.status,
            cargo_name = COALESCE(excluded.cargo_name, career_job_history.cargo_name),
            source_city = COALESCE(excluded.source_city, career_job_history.source_city),
            destination_city = COALESCE(excluded.destination_city, career_job_history.destination_city),
            source_company = COALESCE(excluded.source_company, career_job_history.source_company),
            destination_company = COALESCE(excluded.destination_company, career_job_history.destination_company),
            distance_km = COALESCE(excluded.distance_km, career_job_history.distance_km),
            revenue = COALESCE(excluded.revenue, career_job_history.revenue),
            costs = COALESCE(excluded.costs, career_job_history.costs),
            penalties = COALESCE(excluded.penalties, career_job_history.penalties),
            damage_percent = COALESCE(excluded.damage_percent, career_job_history.damage_percent),
            profit = COALESCE(excluded.profit, career_job_history.profit),
            xp = COALESCE(excluded.xp, career_job_history.xp),
            level_after = COALESCE(excluded.level_after, career_job_history.level_after),
            truck_name = COALESCE(excluded.truck_name, career_job_history.truck_name),
            trailer_name = COALESCE(excluded.trailer_name, career_job_history.trailer_name),
            driven_with_truck = COALESCE(excluded.driven_with_truck, career_job_history.driven_with_truck),
            data_origin_note = COALESCE(excluded.data_origin_note, career_job_history.data_origin_note),
            raw_data_json = COALESCE(excluded.raw_data_json, career_job_history.raw_data_json),
            updated_at = excluded.updated_at
        "#,
        params![
            item.game.clone(),
            item.profile_id.clone(),
            item.profile_name.clone(),
            item.save_path.clone(),
            item.source.clone(),
            item.source_save_name.clone(),
            item.detected_at.clone(),
            item.started_at.clone(),
            item.completed_at.clone(),
            item.job_uid.clone(),
            item.status.clone(),
            item.cargo_name.clone(),
            item.source_city.clone(),
            item.destination_city.clone(),
            item.source_company.clone(),
            item.destination_company.clone(),
            item.distance_km,
            item.revenue,
            item.costs,
            item.penalties,
            item.damage_percent,
            item.profit,
            item.xp,
            item.level_after,
            item.truck_name.clone(),
            item.trailer_name.clone(),
            item.driven_with_truck.map(|value| if value { 1 } else { 0 }),
            item.data_origin_note.clone(),
            item.raw_data_json.clone(),
            now,
            now
        ],
    )
    .map_err(|error| error.to_string())?;

    Ok(())
}

fn get_item_by_uid(conn: &Connection, job_uid: &str) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT id FROM career_job_history WHERE job_uid = ?1",
        [job_uid],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn build_selected_job_record(
    game: &str,
    profile_id: &str,
    profile_name: &str,
    save_dir: &Path,
    save_name: &str,
    detected_at: &str,
    player_xp: Option<i64>,
    job: &save_dispatcher::JobInfoData,
) -> ScannedJobRecord {
    let route = decode_company_pointer(job.source_company.as_deref());
    let destination = decode_company_pointer(job.target_company.as_deref());
    let cargo_name = job.cargo.as_deref().map(pretty_token_value);
    let raw = json!({
        "pointer": job.pointer,
        "cargo": job.cargo,
        "sourceCompany": job.source_company,
        "targetCompany": job.target_company,
        "plannedDistanceKm": job.planned_distance_km,
        "urgency": job.urgency,
        "cargoModelIndex": job.cargo_model_index,
        "isCargoMarketJob": job.is_cargo_market_job,
        "unitsCount": job.units_count,
        "fillRatio": job.fill_ratio
    });

    ScannedJobRecord {
        game: Some(game.to_string()),
        profile_id: profile_id.to_string(),
        profile_name: profile_name.to_string(),
        save_path: save_dir.display().to_string().replace('\\', "/"),
        source: "partial_save_data".to_string(),
        source_save_name: save_name.to_string(),
        detected_at: detected_at.to_string(),
        started_at: None,
        completed_at: None,
        job_uid: stable_scanned_job_uid(
            profile_id,
            "selected",
            job.cargo.as_deref(),
            job.source_company.as_deref(),
            job.target_company.as_deref(),
            job.planned_distance_km,
            job.urgency,
            Some(&job.pointer),
        ),
        status: "unknown".to_string(),
        cargo_name,
        source_city: route.1,
        destination_city: destination.1,
        source_company: route.0,
        destination_company: destination.0,
        distance_km: job.planned_distance_km.map(|value| value as f64),
        revenue: None,
        costs: None,
        penalties: None,
        damage_percent: None,
        profit: None,
        xp: player_xp,
        level_after: None,
        truck_name: None,
        trailer_name: None,
        driven_with_truck: Some(true),
        data_origin_note: Some(
            "Imported from save-selected job reference. Historical completion state is not available in ETS2/ATS save data.".to_string(),
        ),
        raw_data_json: Some(raw.to_string()),
    }
}

fn build_active_job_record(
    game: &str,
    profile_id: &str,
    profile_name: &str,
    save_dir: &Path,
    save_name: &str,
    detected_at: &str,
    player_xp: Option<i64>,
    job: &save_dispatcher::ActiveJobData,
) -> ScannedJobRecord {
    let route = decode_company_pointer(job.source_company.as_deref());
    let destination = decode_company_pointer(job.target_company.as_deref());
    let cargo_name = job.cargo.as_deref().map(pretty_token_value);
    let raw = json!({
        "pointer": job.pointer,
        "companyTruck": job.company_truck,
        "companyTrailer": job.company_trailer,
        "cargo": job.cargo,
        "sourceCompany": job.source_company,
        "targetCompany": job.target_company,
        "plannedDistanceKm": job.planned_distance_km,
        "urgency": job.urgency,
        "totalFines": job.total_fines,
        "timeLowerLimit": job.time_lower_limit,
        "timeUpperLimit": job.time_upper_limit,
        "startTime": job.start_time,
        "isTrailerLoaded": job.is_trailer_loaded,
        "autoloadUsed": job.autoload_used,
        "isCargoMarketJob": job.is_cargo_market_job,
        "selectedTarget": job.selected_target,
        "cargoModelIndex": job.cargo_model_index,
        "unitsCount": job.units_count,
        "fillRatio": job.fill_ratio
    });

    ScannedJobRecord {
        game: Some(game.to_string()),
        profile_id: profile_id.to_string(),
        profile_name: profile_name.to_string(),
        save_path: save_dir.display().to_string().replace('\\', "/"),
        source: "partial_save_data".to_string(),
        source_save_name: save_name.to_string(),
        detected_at: detected_at.to_string(),
        started_at: job.start_time.map(|_| detected_at.to_string()),
        completed_at: None,
        job_uid: stable_scanned_job_uid(
            profile_id,
            "active",
            job.cargo.as_deref(),
            job.source_company.as_deref(),
            job.target_company.as_deref(),
            job.planned_distance_km,
            job.start_time,
            Some(&job.pointer),
        ),
        status: "active".to_string(),
        cargo_name,
        source_city: route.1,
        destination_city: destination.1,
        source_company: route.0,
        destination_company: destination.0,
        distance_km: job.planned_distance_km.map(|value| value as f64),
        revenue: None,
        costs: None,
        penalties: job.total_fines,
        damage_percent: None,
        profit: None,
        xp: player_xp,
        level_after: None,
        truck_name: job.company_truck.as_deref().map(pretty_token_value),
        trailer_name: job.company_trailer.as_deref().map(pretty_token_value),
        driven_with_truck: Some(true),
        data_origin_note: Some(
            "Imported from active save job state. Completion, final payout and full historical context are not stored reliably in ETS2/ATS saves.".to_string(),
        ),
        raw_data_json: Some(raw.to_string()),
    }
}

fn item_primary_date(item: &AnalyticsJobHistoryItem) -> NaiveDate {
    parse_rfc3339_date(item.started_at.as_deref())
        .or_else(|| parse_rfc3339_date(Some(&item.detected_at)))
        .or_else(|| parse_rfc3339_date(item.completed_at.as_deref()))
        .unwrap_or_else(|| Utc::now().date_naive())
}

fn parse_rfc3339_date(value: Option<&str>) -> Option<NaiveDate> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.naive_utc().date())
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        None
    } else {
        Some(normalize_for_match(trimmed))
    }
}

fn normalize_for_match(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn contains_text(haystack: Option<&str>, needle: &str) -> bool {
    haystack
        .map(normalize_for_match)
        .map(|value| value.contains(needle))
        .unwrap_or(false)
}

fn resolve_profile_name(profile_dir: &Path) -> Option<String> {
    let profile_sii = profile_dir.join("profile.sii");
    let content = decrypt_if_needed(&profile_sii).ok()?;
    extract_profile_name(&content)
}

fn resolve_save_name(save_dir: &Path) -> Option<String> {
    let info_sii = save_dir.join("info.sii");
    let content = decrypt_if_needed(&info_sii).ok()?;
    extract_save_name(&content)
}

fn extract_integer_field(content: &str, field: &str) -> Option<i64> {
    let prefix = format!("{field}:");
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix(&prefix) {
            if let Ok(parsed) = value.trim().parse::<i64>() {
                return Some(parsed);
            }
        }
    }
    None
}

fn infer_game(profile_path: &str, selected_game: Option<&str>) -> String {
    let lower = profile_path.to_ascii_lowercase();
    if lower.contains("american truck simulator") || lower.contains("/ats") {
        "ATS".to_string()
    } else if lower.contains("euro truck simulator") || lower.contains("/ets2") {
        "ETS2".to_string()
    } else if selected_game
        .map(|value| value.eq_ignore_ascii_case("ats"))
        .unwrap_or(false)
    {
        "ATS".to_string()
    } else {
        "ETS2".to_string()
    }
}

fn decode_company_pointer(value: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return (None, None);
    };

    let normalized = value
        .strip_prefix("company.volatile.")
        .unwrap_or(value)
        .to_string();
    let mut parts = normalized.split('.').collect::<Vec<_>>();
    if parts.len() >= 2 {
        let city = parts.pop().map(pretty_token_value);
        let company = Some(pretty_token_value(&parts.join(".")));
        (company, city)
    } else {
        (Some(pretty_token_value(&normalized)), None)
    }
}

fn pretty_token_value(value: &str) -> String {
    let compact = value
        .trim()
        .rsplit('.')
        .next()
        .unwrap_or(value)
        .replace('_', " ")
        .replace('-', " ");

    compact
        .split_whitespace()
        .map(|token| {
            let mut chars = token.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = String::new();
                    out.extend(first.to_uppercase());
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn safe_label(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "Unknown".to_string())
}

fn stable_scanned_job_uid(
    profile_id: &str,
    status: &str,
    cargo: Option<&str>,
    source_company: Option<&str>,
    destination_company: Option<&str>,
    distance_km: Option<i64>,
    time_marker: Option<i64>,
    pointer: Option<&str>,
) -> String {
    let fingerprint = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        profile_id,
        status,
        cargo.unwrap_or(""),
        source_company.unwrap_or(""),
        destination_company.unwrap_or(""),
        distance_km.unwrap_or_default(),
        time_marker.unwrap_or_default(),
        pointer.unwrap_or("")
    );

    let mut hasher = Sha256::new();
    hasher.update(fingerprint.as_bytes());
    let digest = hasher.finalize();
    format!("savejob-{:x}", digest)[..24].to_string()
}

fn build_csv(items: &[AnalyticsJobHistoryItem]) -> String {
    let header = [
        "Datum",
        "Profil",
        "Game",
        "Auftrag-ID",
        "Status",
        "Fracht",
        "Start-Stadt",
        "Ziel-Stadt",
        "Firma Start",
        "Firma Ziel",
        "Distanz (km)",
        "Umsatz",
        "Kosten",
        "Schaden (%)",
        "Strafe",
        "Gewinn",
        "XP",
        "Level",
        "Truck",
        "Trailer",
        "Quelle",
        "Save",
    ];

    let mut lines = Vec::with_capacity(items.len() + 1);
    lines.push(header.join(";"));

    for item in items {
        let row = [
            csv_cell(item.started_at.as_deref().unwrap_or(&item.detected_at)),
            csv_cell(
                item.profile_name
                    .as_deref()
                    .or(item.profile_id.as_deref())
                    .unwrap_or(""),
            ),
            csv_cell(item.game.as_deref().unwrap_or("")),
            csv_cell(&item.job_uid),
            csv_cell(&item.status),
            csv_cell(item.cargo_name.as_deref().unwrap_or("")),
            csv_cell(item.source_city.as_deref().unwrap_or("")),
            csv_cell(item.destination_city.as_deref().unwrap_or("")),
            csv_cell(item.source_company.as_deref().unwrap_or("")),
            csv_cell(item.destination_company.as_deref().unwrap_or("")),
            csv_cell_opt_f64(item.distance_km, 1),
            csv_cell_opt_i64(item.revenue),
            csv_cell_opt_i64(item.costs),
            csv_cell_opt_f64(item.damage_percent, 2),
            csv_cell_opt_i64(item.penalties),
            csv_cell_opt_i64(item.profit),
            csv_cell_opt_i64(item.xp),
            csv_cell_opt_i64(item.level_after),
            csv_cell(item.truck_name.as_deref().unwrap_or("")),
            csv_cell(item.trailer_name.as_deref().unwrap_or("")),
            csv_cell(&item.source),
            csv_cell(item.source_save_name.as_deref().unwrap_or("")),
        ];
        lines.push(row.join(";"));
    }

    lines.join("\r\n")
}

fn csv_cell(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

fn csv_cell_opt_i64(value: Option<i64>) -> String {
    csv_cell(&value.map(|item| item.to_string()).unwrap_or_default())
}

fn csv_cell_opt_f64(value: Option<f64>, digits: usize) -> String {
    csv_cell(
        &value
            .map(|item| format!("{:.*}", digits, item).replace('.', ","))
            .unwrap_or_default(),
    )
}
