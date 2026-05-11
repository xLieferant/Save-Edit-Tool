use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use rusqlite::{Connection, params};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::db::sqlite;
use crate::shared::current_profile::snapshot_resolved_save_context;
use crate::shared::decrypt::decrypt_if_needed;
use crate::shared::extract::extract_profile_name;
use crate::shared::extract_save_name::extract_save_name;
use crate::shared::logs;
use crate::shared::paths::{game_sii_from_save, info_sii_from_save};
use crate::shared::user_log;
use crate::state::AppProfileState;

use super::models::{LogContext, RuntimeReportEntry};

const REPORT_EXPORT_NAME: &str = "ets2_tool_logs.txt";

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
        extra.insert("saveInferred".to_string(), resolved.save_inferred.to_string());
    }

    let profile_reference = resolved
        .as_ref()
        .and_then(|item| item.context.profile_reference.clone());
    let save_reference = resolved
        .as_ref()
        .and_then(|item| item.context.save_reference.clone());

    LogContext {
        selected_game,
        profile_name: profile_reference
            .as_deref()
            .and_then(resolve_profile_name_from_path),
        save_name: save_reference.as_deref().and_then(resolve_save_name_from_path),
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

pub fn build_support_report(profile_state: &AppProfileState) -> Result<String, String> {
    let context = resolve_active_context(profile_state);
    let entries = recent_entries(30)?;
    let generated_at = Utc::now().to_rfc3339();

    let mut lines = Vec::new();
    lines.push("ETS2 Tool Support Report".to_string());
    lines.push("=======================".to_string());
    lines.push(format!("Generated at: {}", generated_at));
    lines.push(format!(
        "Game: {}",
        context
            .selected_game
            .as_deref()
            .unwrap_or("unknown")
            .to_uppercase()
    ));
    lines.push(format!(
        "Profile: {}",
        context.profile_name.as_deref().unwrap_or("-")
    ));
    lines.push(format!("Save: {}", context.save_name.as_deref().unwrap_or("-")));
    lines.push(format!(
        "Profile reference: {}",
        context.profile_reference.as_deref().unwrap_or("-")
    ));
    lines.push(format!(
        "Save reference: {}",
        context.save_reference.as_deref().unwrap_or("-")
    ));
    lines.push(String::new());

    lines.push("Recent Runtime Reports".to_string());
    lines.push("---------------------".to_string());
    if entries.is_empty() {
        lines.push("No runtime reports recorded yet.".to_string());
    } else {
        for entry in entries {
            lines.push(format!(
                "[{}] {} | {} | profile={} | save={} | code={}",
                entry.created_at_utc,
                entry.level.to_uppercase(),
                entry.action,
                entry.profile_name.as_deref().unwrap_or("-"),
                entry.save_name.as_deref().unwrap_or("-"),
                entry.error_code.as_deref().unwrap_or("-")
            ));
            lines.push(format!("User message: {}", entry.user_message));
            if let Some(details) = entry.technical_details.as_deref() {
                lines.push(format!("Technical details: {}", details));
            }
            if !entry.context.extra.is_empty() {
                lines.push("Context:".to_string());
                for (key, value) in entry.context.extra {
                    lines.push(format!("  - {}: {}", key, value));
                }
            }
            lines.push(String::new());
        }
    }

    lines.push("Log Files".to_string());
    lines.push("---------".to_string());
    lines.push(format!(
        "Technical log: {}",
        redact_path(&logs::technical_log_path().display().to_string())
    ));
    lines.push(format!(
        "User log: {}",
        redact_path(&user_log::user_log_path().display().to_string())
    ));

    Ok(lines.join("\r\n"))
}

pub fn export_logs_bundle(app: &AppHandle, profile_state: &AppProfileState) -> Result<Option<String>, String> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("Text file", &["txt"])
        .set_title("Export logs")
        .set_file_name(REPORT_EXPORT_NAME)
        .blocking_save_file();

    let Some(file_path) = file_path else {
        return Ok(None);
    };

    let path = file_path
        .into_path()
        .map_err(|_| "The selected export path could not be resolved.".to_string())?;

    let mut body = build_support_report(profile_state)?;
    let technical = fs::read_to_string(logs::technical_log_path()).unwrap_or_default();
    let user = fs::read_to_string(user_log::user_log_path()).unwrap_or_default();

    body.push_str("\r\n\r\nTechnical Log\r\n-------------\r\n");
    body.push_str(&technical);
    body.push_str("\r\n\r\nUser Log\r\n--------\r\n");
    body.push_str(&user);

    fs::write(&path, body).map_err(|error| {
        format!(
            "The log export could not be written to {}: {}",
            path.display(),
            error
        )
    })?;

    Ok(Some(path.display().to_string()))
}

fn record_entry(
    level: &str,
    action: &str,
    error_code: Option<&str>,
    user_message: &str,
    technical_details: Option<&str>,
    context: &LogContext,
) -> Result<(), String> {
    let report_id = format!("report-{}", Uuid::new_v4());
    let created_at_utc = Utc::now().to_rfc3339();
    let context_json = serde_json::to_string(context).map_err(|error| error.to_string())?;

    let conn = open_runtime_connection()?;
    conn.execute(
        r#"
        INSERT INTO ets_runtime_reports (
            report_id, created_at_utc, level, action, profile_name, save_name,
            error_code, user_message, technical_details, context_json
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
        params![
            report_id,
            created_at_utc,
            level,
            action,
            context.profile_name,
            context.save_name,
            error_code,
            user_message,
            technical_details,
            context_json
        ],
    )
    .map_err(|error| error.to_string())?;

    let redacted_profile = context.profile_reference.as_deref().unwrap_or("-");
    let redacted_save = context.save_reference.as_deref().unwrap_or("-");
    let line = format!(
        "[runtime:{}] action={} code={} profile={} save={} user={} technical={}",
        level,
        action,
        error_code.unwrap_or("-"),
        redacted_profile,
        redacted_save,
        user_message,
        technical_details.unwrap_or("-")
    );
    crate::dev_log!("{}", line);

    if level == "error" || level == "warning" {
        let _ = user_log::write_user_log(
            &format!("{} | {}", action, user_message),
            if level == "error" { "error" } else { "warning" },
        );
    }

    Ok(())
}

fn open_runtime_connection() -> Result<Connection, String> {
    let db_path = sqlite::app_db_path();
    let conn = Connection::open(db_path).map_err(|error| error.to_string())?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    Ok(conn)
}

fn resolve_profile_name_from_path(profile_path: &str) -> Option<String> {
    let sii_path = Path::new(profile_path).join("profile.sii");
    let content = decrypt_if_needed(&sii_path).ok()?;
    extract_profile_name(&content)
        .or_else(|| {
            Path::new(profile_path)
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
}

fn resolve_save_name_from_path(save_path: &str) -> Option<String> {
    let info_path = info_sii_from_save(Path::new(save_path));
    if let Ok(content) = decrypt_if_needed(&info_path) {
        if let Some(name) = extract_save_name(&content) {
            return Some(name);
        }
    }

    let game_path = game_sii_from_save(Path::new(save_path));
    if let Ok(content) = decrypt_if_needed(&game_path) {
        if let Some(name) = extract_save_name(&content) {
            return Some(name);
        }
    }

    Path::new(save_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
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
