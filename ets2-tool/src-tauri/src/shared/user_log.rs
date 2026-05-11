use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::shared::logs;

const USER_LOG_FILE_NAME: &str = "ets2_tool_user.log";
const ERROR_LOG_FILE_NAME: &str = "ets2_tool_errors.log";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLogEntry {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
    pub raw_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLogStatus {
    pub log_file_path: String,
    pub debug_log_path: String,
    pub error_log_path: String,
    pub log_size_bytes: u64,
    pub last_modified_utc: Option<String>,
    pub warning_count: u32,
    pub error_count: u32,
}

pub fn user_log_path() -> PathBuf {
    logs::log_directory_path()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(USER_LOG_FILE_NAME)
}

pub fn debug_log_path() -> PathBuf {
    logs::technical_log_path()
}

pub fn error_log_path() -> PathBuf {
    logs::log_directory_path()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(ERROR_LOG_FILE_NAME)
}

fn normalize_level(level: &str) -> String {
    match level.trim().to_ascii_uppercase().as_str() {
        "WARNING" => "WARN".to_string(),
        "WARN" => "WARN".to_string(),
        "ERROR" => "ERROR".to_string(),
        "DEBUG" => "DEBUG".to_string(),
        _ => "INFO".to_string(),
    }
}

fn format_entry(level: &str, module: &str, message: &str) -> String {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    format!(
        "[{}] [{}] [{}] {}\n",
        timestamp,
        normalize_level(level),
        module.trim(),
        message.trim()
    )
}

fn write_entry(level: &str, module: &str, message: &str) -> Result<(), String> {
    logs::ensure_log_directory()?;
    let normalized_level = normalize_level(level);
    let entry = format_entry(&normalized_level, module, message);

    match normalized_level.as_str() {
        "DEBUG" => {
            logs::append_log_line(&debug_log_path(), &entry)?;
        }
        "ERROR" => {
            logs::append_log_line(&user_log_path(), &entry)?;
            logs::append_log_line(&error_log_path(), &entry)?;
        }
        _ => {
            logs::append_log_line(&user_log_path(), &entry)?;
        }
    }

    Ok(())
}

pub fn user_log_info(module: &str, message: impl AsRef<str>) -> Result<(), String> {
    write_entry("INFO", module, message.as_ref())
}

pub fn user_log_warn(module: &str, message: impl AsRef<str>) -> Result<(), String> {
    write_entry("WARN", module, message.as_ref())
}

pub fn user_log_error(module: &str, message: impl AsRef<str>) -> Result<(), String> {
    write_entry("ERROR", module, message.as_ref())
}

pub fn user_log_debug(module: &str, message: impl AsRef<str>) -> Result<(), String> {
    write_entry("DEBUG", module, message.as_ref())
}

fn humanize_token(value: &str) -> String {
    value
        .split(['_', '-', ':', '/'])
        .filter(|part| !part.trim().is_empty())
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
        .join(" ")
}

pub fn write_user_log(action: &str, stage: &str) -> Result<(), String> {
    let normalized_stage = stage.trim().to_ascii_lowercase();
    let module = "Frontend";
    let message = format!(
        "{} ({})",
        humanize_token(action.trim()),
        if normalized_stage.is_empty() {
            "info".to_string()
        } else {
            normalized_stage.clone()
        }
    );

    match normalized_stage.as_str() {
        "warning" | "warn" => user_log_warn(module, message),
        "error" | "failed" | "fail" => user_log_error(module, message),
        "debug" => user_log_debug(module, message),
        _ => user_log_info(module, message),
    }
}

fn parse_log_line(line: &str) -> Option<UserLogEntry> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (timestamp, rest) = trimmed.strip_prefix('[')?.split_once("] ")?;
    let (level, rest) = rest.strip_prefix('[')?.split_once("] ")?;
    let (module, message) = rest.strip_prefix('[')?.split_once("] ")?;

    Some(UserLogEntry {
        timestamp: timestamp.to_string(),
        level: normalize_level(level),
        module: module.to_string(),
        message: message.to_string(),
        raw_line: trimmed.to_string(),
    })
}

pub fn get_user_logs(
    level_filter: Option<&str>,
    max_lines: Option<u32>,
) -> Result<Vec<UserLogEntry>, String> {
    logs::ensure_log_file(&user_log_path())?;
    let content = fs::read_to_string(user_log_path()).unwrap_or_default();
    let normalized_filter = level_filter.map(normalize_level);
    let limit = max_lines.unwrap_or(400) as usize;

    let mut entries = content
        .lines()
        .filter_map(parse_log_line)
        .filter(|entry| {
            normalized_filter
                .as_deref()
                .map(|filter| entry.level == filter)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();

    entries.reverse();
    if entries.len() > limit {
        entries.truncate(limit);
    }

    Ok(entries)
}

fn modified_to_utc(value: SystemTime) -> String {
    let datetime = DateTime::<Utc>::from(value);
    datetime.to_rfc3339()
}

pub fn get_log_status() -> Result<UserLogStatus, String> {
    logs::ensure_log_file(&user_log_path())?;
    logs::ensure_log_file(&debug_log_path())?;
    logs::ensure_log_file(&error_log_path())?;

    let entries = get_user_logs(None, Some(10_000))?;
    let warning_count = entries.iter().filter(|entry| entry.level == "WARN").count() as u32;
    let error_count = entries.iter().filter(|entry| entry.level == "ERROR").count() as u32;
    let metadata = fs::metadata(user_log_path())
        .map_err(|error| format!("Could not read user log metadata: {}", error))?;

    Ok(UserLogStatus {
        log_file_path: user_log_path().display().to_string(),
        debug_log_path: debug_log_path().display().to_string(),
        error_log_path: error_log_path().display().to_string(),
        log_size_bytes: metadata.len(),
        last_modified_utc: metadata.modified().ok().map(modified_to_utc),
        warning_count,
        error_count,
    })
}

fn build_clear_backup_path() -> PathBuf {
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    logs::log_directory_path()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(format!("ets2_tool_user_logs_before_clear_{}.txt", timestamp))
}

fn clear_single_log(path: &Path) -> Result<(), String> {
    logs::ensure_log_file(path)?;
    fs::write(path, "")
        .map_err(|error| format!("Could not clear log file {}: {}", path.display(), error))?;
    logs::clear_log_archives(path)?;
    Ok(())
}

pub fn clear_user_logs() -> Result<Option<String>, String> {
    logs::ensure_log_directory()?;
    let user_path = user_log_path();
    let debug_path = debug_log_path();
    let error_path = error_log_path();

    let user_content = fs::read_to_string(&user_path).unwrap_or_default();
    let error_content = fs::read_to_string(&error_path).unwrap_or_default();
    let debug_content = fs::read_to_string(&debug_path).unwrap_or_default();

    let backup_path = if user_content.trim().is_empty()
        && error_content.trim().is_empty()
        && debug_content.trim().is_empty()
    {
        None
    } else {
        let path = build_clear_backup_path();
        let body = [
            "ETS2 Tool Logs Backup Before Clear".to_string(),
            "=================================".to_string(),
            format!("Created at: {}", Utc::now().to_rfc3339()),
            String::new(),
            "User Log".to_string(),
            "--------".to_string(),
            user_content,
            String::new(),
            "Error Log".to_string(),
            "---------".to_string(),
            error_content,
            String::new(),
            "Debug Log".to_string(),
            "---------".to_string(),
            debug_content,
        ]
        .join("\r\n");

        fs::write(&path, body).map_err(|error| {
            format!(
                "Could not create user log backup {}: {}",
                path.display(),
                error
            )
        })?;
        Some(path.display().to_string())
    };

    clear_single_log(&user_path)?;
    clear_single_log(&error_path)?;
    clear_single_log(&debug_path)?;

    let _ = user_log_info("UserLogs", "User logs were cleared by the user.");

    Ok(backup_path)
}
