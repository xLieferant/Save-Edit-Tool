use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;
use tauri::AppHandle;

use crate::db::sqlite;
use crate::shared::current_profile::snapshot_resolved_save_context;
use crate::shared::logs;
use crate::shared::user_log::{self, UserLogEntry, UserLogStatus};
use crate::state::AppProfileState;

use super::models::{LogContext, RuntimeReportEntry};

pub fn resolve_active_context(profile_state: &AppProfileState) -> LogContext {
    let resolved = snapshot_resolved_save_context(profile_state).ok();
    let selected_game = profile_state
        .selected_game
        .lock()
        .ok()
        .map(|guard| guard.clone());

    let mut extra = BTreeMap::new();
    if let Some(resolved) = resolved.as_ref() {
        extra.insert(
            "profileInferred".to_string(),
            resolved.profile_inferred.to_string(),
        );
        extra.insert(
            "saveInferred".to_string(),
            resolved.save_inferred.to_string(),
        );
    }

    let profile_reference = resolved
        .as_ref()
        .and_then(|item| item.context.profile_reference.clone());
    let save_reference = resolved
        .as_ref()
        .and_then(|item| item.context.save_reference.clone());

    LogContext {
        selected_game,
        // Logging must not decrypt saves or collect personal names just to build context.
        profile_name: None,
        save_name: None,
        profile_reference: profile_reference.as_deref().map(redact_path),
        save_reference: save_reference.as_deref().map(redact_path),
        extra,
    }
}

pub fn record_info(action: &str, user_message: &str, context: &LogContext) -> Result<(), String> {
    record_entry("info", action, None, user_message, None, context)
}

pub fn record_warning(
    action: &str,
    error_code: Option<&str>,
    user_message: &str,
    technical_details: Option<&str>,
    context: &LogContext,
) -> Result<(), String> {
    record_entry(
        "warning",
        action,
        error_code,
        user_message,
        technical_details,
        context,
    )
}

pub fn record_error(
    action: &str,
    error_code: Option<&str>,
    user_message: &str,
    technical_details: Option<&str>,
    context: &LogContext,
) -> Result<(), String> {
    record_entry(
        "error",
        action,
        error_code,
        user_message,
        technical_details,
        context,
    )
}

pub fn recent_entries(limit: usize) -> Result<Vec<RuntimeReportEntry>, String> {
    let conn = open_runtime_connection()?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT report_id, created_at_utc, level, action, profile_name, save_name,
                   error_code, user_message, technical_details, context_json
            FROM ets_runtime_reports
            ORDER BY created_at_utc DESC
            LIMIT ?1
            "#,
        )
        .map_err(|error| error.to_string())?;

    let rows = stmt
        .query_map([limit as i64], |row| {
            let context_json: String = row.get("context_json")?;
            let context = serde_json::from_str::<LogContext>(&context_json).unwrap_or_default();
            Ok(RuntimeReportEntry {
                report_id: row.get("report_id")?,
                created_at_utc: row.get("created_at_utc")?,
                level: row.get("level")?,
                action: row.get("action")?,
                profile_name: row.get("profile_name")?,
                save_name: row.get("save_name")?,
                error_code: row.get("error_code")?,
                user_message: row.get("user_message")?,
                technical_details: row.get("technical_details")?,
                context,
            })
        })
        .map_err(|error| error.to_string())?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.map_err(|error| error.to_string())?);
    }
    Ok(entries)
}

pub fn get_user_logs(
    level_filter: Option<String>,
    max_lines: Option<u32>,
) -> Result<Vec<UserLogEntry>, String> {
    user_log::get_user_logs(level_filter.as_deref(), max_lines)
}

pub fn get_log_status() -> Result<UserLogStatus, String> {
    user_log::get_log_status()
}

pub fn clear_user_logs() -> Result<Option<String>, String> {
    user_log::clear_user_logs()
}

pub fn build_support_report(_profile_state: &AppProfileState) -> Result<String, String> {
    logs::diagnostic::active()
        .ok_or("diagnostics.errors.unavailable")?
        .read_text()
}

pub fn export_user_logs(
    app: &AppHandle,
    _profile_state: &AppProfileState,
) -> Result<String, String> {
    let runtime = logs::diagnostic::active().ok_or("diagnostics.errors.unavailable")?;
    super::report::export(&runtime, app).map(|path| path.unwrap_or_default())
}

pub fn export_logs_bundle(
    app: &AppHandle,
    _profile_state: &AppProfileState,
) -> Result<Option<String>, String> {
    let runtime = logs::diagnostic::active().ok_or("diagnostics.errors.unavailable")?;
    super::report::export(&runtime, app)
}

fn log_to_user_log(level: &str, action: &str, user_message: &str) -> Result<(), String> {
    let module = action
        .split(['_', '-', ':'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut token = first.to_uppercase().to_string();
                    token.push_str(chars.as_str());
                    token
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    match level {
        "error" => user_log::user_log_error(&module, user_message),
        "warning" => user_log::user_log_warn(&module, user_message),
        _ => user_log::user_log_info(&module, user_message),
    }
}

fn record_entry(
    level: &str,
    action: &str,
    error_code: Option<&str>,
    user_message: &str,
    technical_details: Option<&str>,
    context: &LogContext,
) -> Result<(), String> {
    if let Some(runtime) = logs::diagnostic::active() {
        let level = match level {
            "warning" => "WARN",
            "error" => "ERROR",
            _ => "INFO",
        };
        let mut event =
            logs::diagnostic::DiagnosticEvent::new(level, "save_editor", action, user_message);
        event.technical = technical_details.map(str::to_owned);
        event.error_kind = error_code.map(str::to_owned);
        event.context = context.extra.clone();
        event.backup_id = context.extra.get("backupId").cloned();
        runtime.record(event);
        return Ok(());
    }
    let detail = technical_details
        .map(|cause| format!("{}\nReason: {}", user_message, cause))
        .unwrap_or_else(|| user_message.to_owned());
    log_to_user_log(level, action, &detail)
}

fn open_runtime_connection() -> Result<Connection, String> {
    let db_path = sqlite::app_db_path();
    let conn = Connection::open(db_path).map_err(|error| error.to_string())?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    Ok(conn)
}

pub fn redact_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    for needle in ["/profiles/", "/profiles.backup/", "/save/", "/mod/"] {
        if let Some(index) = normalized.to_ascii_lowercase().find(needle) {
            return format!("...{}", &normalized[index..]);
        }
    }

    PathBuf::from(normalized)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}
