//! Compatibility export adapter: native Save dialog, full current TXT log only.
use crate::shared::logs::diagnostic::{
    DiagnosticRuntime, LOG_FILE, atomic_write, checked_path, redact_log_text,
};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use regex::Regex;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLogRecord {
    pub timestamp: Option<String>,
    pub level: Option<String>,
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLogSnapshot {
    pub revision: String,
    pub log_file_path: String,
    pub size_bytes: u64,
    pub last_modified: Option<String>,
    pub warning_count: usize,
    pub error_count: usize,
    pub entries: Vec<UserLogRecord>,
}

/// Keep continuation lines with their original entry, including technical causes and stacks.
pub fn parse_user_logs(text: &str) -> Vec<UserLogRecord> {
    let header = Regex::new(r"(?m)^\[([^\]\r\n]+)\][ \t]+\[([A-Z]+)\][ \t]+\[[^\]\r\n]*\]")
        .expect("valid log header regex");
    let matches = header.captures_iter(text).collect::<Vec<_>>();
    let mut entries = Vec::new();
    let first = matches
        .first()
        .map(|capture| capture.get(0).unwrap().start())
        .unwrap_or(text.len());
    if !text[..first].trim().is_empty() {
        entries.push(UserLogRecord {
            timestamp: None,
            level: None,
            text: text[..first].to_string(),
        });
    }
    for (index, capture) in matches.iter().enumerate() {
        let start = capture.get(0).unwrap().start();
        let end = matches
            .get(index + 1)
            .map(|next| next.get(0).unwrap().start())
            .unwrap_or(text.len());
        entries.push(UserLogRecord {
            timestamp: Some(capture[1].to_string()),
            level: Some(capture[2].to_string()),
            text: text[start..end].to_string(),
        });
    }
    entries
}

fn timestamp_millis(record: &UserLogRecord) -> Option<i64> {
    let text = record.timestamp.as_deref()?;
    DateTime::parse_from_rfc3339(text)
        .map(|time| time.timestamp_millis())
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S")
                .ok()
                .and_then(|time| Local.from_local_datetime(&time).earliest())
                .map(|time| time.timestamp_millis())
        })
}

pub fn user_log_snapshot(
    runtime: &DiagnosticRuntime,
    known_revision: Option<&str>,
) -> Result<Option<UserLogSnapshot>, String> {
    let Some(snapshot) = runtime.read_text_snapshot(known_revision)? else {
        return Ok(None);
    };
    let mut entries = parse_user_logs(&redact_log_text(&snapshot.text));
    let warning_count = entries
        .iter()
        .filter(|entry| matches!(entry.level.as_deref(), Some("WARN" | "WARNING")))
        .count();
    let error_count = entries
        .iter()
        .filter(|entry| matches!(entry.level.as_deref(), Some("ERROR" | "FATAL")))
        .count();
    // Reverse first so equal timestamps retain newest-file-entry-first order.
    entries.reverse();
    entries.sort_by_key(|record| std::cmp::Reverse(timestamp_millis(record)));
    Ok(Some(UserLogSnapshot {
        revision: snapshot.revision,
        log_file_path: runtime.root.join(LOG_FILE).display().to_string(),
        size_bytes: snapshot.size_bytes,
        last_modified: snapshot
            .modified
            .map(|time| DateTime::<Utc>::from(time).to_rfc3339()),
        warning_count,
        error_count,
        entries,
    }))
}

pub fn user_report_filename(exported_at: &DateTime<Local>) -> String {
    format!(
        "SaveEditTool-UserLogs_{}.log",
        exported_at.format("%Y-%m-%d_%H-%M-%S")
    )
}

pub fn build_user_report(
    text: &str,
    version: &str,
    operating_system: &str,
    exported_at: &DateTime<Local>,
) -> String {
    let count = parse_user_logs(text)
        .iter()
        .filter(|entry| entry.timestamp.is_some())
        .count();
    format!(
        "================================================\nSAVE EDIT TOOL – USER LOGS\n================================================\n\nExport Date: {}\nExport Time: {}\n\nApplication: ETS2 Save Editor – Alex Edition\nVersion: {}\nOperating System: {}\nLog Entries: {}\n\n================================================\nLOG ENTRIES\n================================================\n\n{}",
        exported_at.format("%Y-%m-%d"),
        exported_at.format("%H:%M:%S"),
        version,
        operating_system,
        count,
        redact_log_text(text)
    )
}

/// create_new protects the source and every existing destination even during name collisions.
pub fn save_user_report(
    runtime: &DiagnosticRuntime,
    desired: &Path,
    content: &str,
) -> Result<PathBuf, String> {
    if !desired
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("log"))
    {
        return Err("user_log_report.errors.log_only".into());
    }
    let parent = desired.parent().ok_or("diagnostics.errors.unsafe_path")?;
    let filename = desired
        .file_name()
        .ok_or("diagnostics.errors.unsafe_path")?;
    checked_path(parent, Path::new(filename))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| "user_log_report.errors.write_failed")?;
    let canonical_root = runtime
        .root
        .canonicalize()
        .map_err(|_| "diagnostics.errors.unsafe_path")?;
    if canonical_parent.starts_with(canonical_root) {
        return Err("diagnostics.errors.unsafe_path".into());
    }
    let stem = desired
        .file_stem()
        .ok_or("diagnostics.errors.unsafe_path")?
        .to_string_lossy();
    for suffix in 0..10_000 {
        let name = if suffix == 0 {
            filename.to_os_string()
        } else {
            format!("{stem}_{suffix}.log").into()
        };
        let destination = checked_path(&canonical_parent, Path::new(&name))?;
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("user_log_report.errors.write_failed".into()),
        };
        let result = file
            .write_all(content.as_bytes())
            .and_then(|_| file.sync_all());
        drop(file);
        if result.is_err() {
            // This path was just reserved with create_new; it cannot be an original log.
            let _ = fs::remove_file(&destination);
            return Err("user_log_report.errors.write_failed".into());
        }
        return Ok(destination);
    }
    Err("user_log_report.errors.write_failed".into())
}

pub fn export_user_report(
    runtime: &DiagnosticRuntime,
    app: &AppHandle,
) -> Result<Option<String>, String> {
    let text = runtime
        .read_text()
        .map_err(|_| "user_log_report.errors.read_failed")?;
    let exported_at = Local::now();
    let filename = user_report_filename(&exported_at);
    let content = build_user_report(
        &text,
        &app.package_info().version.to_string(),
        &format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH),
        &exported_at,
    );
    save_report_with_fallback(
        runtime,
        dirs::download_dir().as_deref(),
        &filename,
        &content,
        || {
            let selected = app
                .dialog()
                .file()
                .add_filter("LOG", &["log"])
                .set_title(crate::features::language::translator::t(
                    "user_log_report.export",
                ))
                .set_file_name(&filename)
                .blocking_save_file();
            selected
                .map(|file| {
                    file.into_path()
                        .map_err(|_| "diagnostics.errors.unsafe_path".to_string())
                })
                .transpose()
        },
    )
}

fn save_report_with_fallback(
    runtime: &DiagnosticRuntime,
    downloads: Option<&Path>,
    filename: &str,
    content: &str,
    picker: impl FnOnce() -> Result<Option<PathBuf>, String>,
) -> Result<Option<String>, String> {
    if let Some(downloads) = downloads {
        if let Ok(destination) = save_user_report(runtime, &downloads.join(filename), content) {
            return Ok(Some(destination.display().to_string()));
        }
    }
    let Some(mut destination) = picker()? else {
        return Ok(None);
    };
    if destination.extension().is_none() {
        destination.set_extension("log");
    }
    let destination = save_user_report(runtime, &destination, content)?;
    Ok(Some(destination.display().to_string()))
}

pub fn save_text(runtime: &DiagnosticRuntime, destination: &Path) -> Result<(), String> {
    if !destination
        .extension()
        .is_some_and(|s| s.to_str().is_some_and(|s| s.eq_ignore_ascii_case("txt")))
    {
        return Err("simple_logging.errors.txt_only".into());
    }
    // Replacing the active log, settings, backups or other application data is never an export.
    let parent = destination
        .parent()
        .ok_or("diagnostics.errors.unsafe_path")?;
    let name = destination
        .file_name()
        .ok_or("diagnostics.errors.unsafe_path")?;
    let destination = checked_path(parent, Path::new(name))?;
    let canonical_parent = parent.canonicalize().map_err(|e| e.to_string())?;
    let canonical_root = runtime.root.canonicalize().map_err(|e| e.to_string())?;
    if canonical_parent.starts_with(&canonical_root) {
        return Err("diagnostics.errors.unsafe_path".into());
    }
    let text = runtime.read_text()?;
    atomic_write(&destination, text.as_bytes())
}

pub fn export(runtime: &DiagnosticRuntime, app: &AppHandle) -> Result<Option<String>, String> {
    let selected = app
        .dialog()
        .file()
        .add_filter("TXT", &["txt"])
        .set_title(crate::features::language::translator::t(
            "simple_logging.export",
        ))
        .set_file_name("ETS2-Toolbox-application.log.txt")
        .blocking_save_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let mut destination = selected
        .into_path()
        .map_err(|_| "diagnostics.errors.unsafe_path")?;
    if destination.extension().is_none() {
        destination.set_extension("txt");
    }
    save_text(runtime, &destination)?;
    Ok(Some(destination.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::logs::diagnostic::DiagnosticEvent;
    use crate::shared::logs::diagnostic::LOG_FILE;
    use std::fs;

    fn export_time() -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 11, 14, 11, 46, 32)
            .single()
            .unwrap()
    }

    fn report_runtime() -> (PathBuf, std::sync::Arc<DiagnosticRuntime>) {
        let directory =
            std::env::temp_dir().join(format!("ets2_user_report_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let runtime = DiagnosticRuntime::start_isolated(directory.join("support"));
        runtime.flush().unwrap();
        (directory, runtime)
    }

    #[test]
    fn user_report_filename_uses_local_date_and_seconds() {
        assert_eq!(
            user_report_filename(&export_time()),
            "SaveEditTool-UserLogs_2026-11-14_11-46-32.log"
        );
    }

    #[test]
    fn complete_user_report_retains_all_levels_and_continuation_lines() {
        let mut text = String::new();
        for index in 0..1605 {
            let level = ["INFO", "WARN", "ERROR"][index % 3];
            text.push_str(&format!("[2026-11-14T11:46:32+01:00] [{level}] [test::export] entry-{index}\n  Reason: original cause {index}\n"));
        }
        let report = build_user_report(&text, "installed-test-version", "test OS", &export_time());
        assert!(report.contains("Log Entries: 1605"));
        assert!(report.contains("Version: installed-test-version"));
        assert!(report.contains("Export Date: 2026-11-14\nExport Time: 11:46:32"));
        assert!(report.contains("Operating System: test OS"));
        assert!(report.ends_with(&text));
        assert!(report.contains("entry-0\n"));
        assert!(report.contains("entry-1604\n"));
    }

    #[test]
    fn snapshot_reads_disk_counts_all_entries_and_sorts_newest_first() {
        let (_, runtime) = report_runtime();
        let text = "[2026-11-14T12:00:00+01:00] [WARN] [test] first\n  Reason: cause\n[2026-11-14T11:00:00+00:00] [INFO] [test] same instant, later entry\n[2026-11-14T14:00:00+01:00] [ERROR] [test] newest\n[2026-11-14T10:00:00+01:00] [INFO] [test] oldest\n";
        fs::write(runtime.root.join(LOG_FILE), text).unwrap();
        let snapshot = user_log_snapshot(&runtime, None).unwrap().unwrap();
        assert_eq!(snapshot.entries.len(), 4);
        assert_eq!(snapshot.warning_count, 1);
        assert_eq!(snapshot.error_count, 1);
        assert_eq!(snapshot.size_bytes, text.len() as u64);
        assert!(snapshot.entries[0].text.contains("newest"));
        assert!(snapshot.entries[1].text.contains("same instant"));
        assert!(snapshot.entries[2].text.contains("Reason: cause"));
        assert!(snapshot.entries[3].text.contains("oldest"));
        assert!(
            user_log_snapshot(&runtime, Some(&snapshot.revision))
                .unwrap()
                .is_none()
        );
        fs::write(
            runtime.root.join(LOG_FILE),
            format!("{text}[2026-11-14T15:00:00+01:00] [ERROR] [test] added\n"),
        )
        .unwrap();
        let changed = user_log_snapshot(&runtime, Some(&snapshot.revision))
            .unwrap()
            .unwrap();
        assert_eq!(changed.error_count, 2);
        assert!(changed.entries[0].text.contains("added"));
        runtime.shutdown();
    }

    #[test]
    fn one_click_export_is_complete_and_never_replaces_existing_files() {
        let (directory, runtime) = report_runtime();
        let original = runtime.read_text().unwrap();
        let filename = user_report_filename(&export_time());
        let body = build_user_report(&original, "test-version", "windows", &export_time());
        let first = save_report_with_fallback(&runtime, Some(&directory), &filename, &body, || {
            panic!("Save dialog must not open when Downloads is writable")
        })
        .unwrap()
        .unwrap();
        let second =
            save_report_with_fallback(&runtime, Some(&directory), &filename, &body, || {
                panic!("A collision must create a unique file")
            })
            .unwrap()
            .unwrap();
        assert_ne!(first, second);
        assert!(second.ends_with("_1.log"));
        assert_eq!(fs::read_to_string(first).unwrap(), body);
        assert_eq!(fs::read_to_string(second).unwrap(), body);
        assert_eq!(runtime.read_text().unwrap(), original);
        assert!(
            save_user_report(&runtime, &runtime.root.join("logs/forbidden.log"), &body).is_err()
        );
        assert!(save_user_report(&runtime, &directory.join("invalid.txt"), &body).is_err());
        runtime.shutdown();
    }

    #[test]
    fn inaccessible_downloads_uses_picker_and_cancel_is_not_success() {
        let (directory, runtime) = report_runtime();
        let blocked = directory.join("not-a-directory");
        fs::write(&blocked, "preserved").unwrap();
        let saved = save_report_with_fallback(
            &runtime,
            Some(&blocked),
            "report.log",
            "complete UTF-8 Grüße",
            || Ok(Some(directory.join("fallback"))),
        )
        .unwrap()
        .unwrap();
        assert!(saved.ends_with("fallback.log"));
        assert_eq!(fs::read_to_string(saved).unwrap(), "complete UTF-8 Grüße");
        assert_eq!(fs::read_to_string(&blocked).unwrap(), "preserved");
        assert!(
            save_report_with_fallback(&runtime, Some(&blocked), "report.log", "body", || Ok(None))
                .unwrap()
                .is_none()
        );
        assert!(
            save_report_with_fallback(&runtime, None, "report.log", "body", || Ok(Some(
                blocked.join("bad.log")
            )))
            .is_err()
        );
        runtime.shutdown();
    }

    #[test]
    fn report_redacts_secrets_without_truncating_the_full_file() {
        let text = format!(
            "[2026-11-14T11:46:32+01:00] [ERROR] [test] permission denied\n{}\npassword=private-password\naccess_token=private-token\nAuthorization: Basic cHJpdmF0ZTpwYXNzd29yZA==\nhttps://user:private-url-password@example.com/\n-----BEGIN PRIVATE KEY-----\nprivate-key-material\n-----END PRIVATE KEY-----\n[2026-11-14T11:46:33+01:00] [WARN] [test] FINAL-ENTRY-Grüße\n",
            "context ".repeat(4000)
        );
        let body = build_user_report(&text, "test-version", "windows", &export_time());
        for secret in [
            "private-password",
            "private-token",
            "cHJpdmF0ZTpwYXNzd29yZA",
            "private-url-password",
            "private-key-material",
        ] {
            assert!(!body.contains(secret), "Secret was exported: {secret}");
        }
        assert!(body.contains("permission denied"));
        assert!(body.contains("FINAL-ENTRY-Grüße"));
        assert!(body.len() > crate::shared::logs::diagnostic::TEXT_LIMIT);
        assert!(body.contains("Log Entries: 2"));
    }

    #[test]
    fn unreadable_utf8_is_reported_without_replacing_the_source() {
        let (_, runtime) = report_runtime();
        let invalid = [0xff, 0xfe, 0x00];
        fs::write(runtime.root.join(LOG_FILE), invalid).unwrap();
        assert!(user_log_snapshot(&runtime, None).is_err());
        assert_eq!(fs::read(runtime.root.join(LOG_FILE)).unwrap(), invalid);
        runtime.shutdown();
    }
    #[test]
    fn txt_export_contains_full_file_and_preserves_source() {
        let directory =
            std::env::temp_dir().join(format!("ets2_logging_export_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let runtime = DiagnosticRuntime::start_isolated(directory.join("support"));
        for n in 0..160 {
            runtime.record(DiagnosticEvent::new(
                "INFO",
                "test",
                "export",
                &format!("line-{n}"),
            ));
        }
        let before = runtime.read_text().unwrap();
        let destination = directory.join("export.txt");
        save_text(&runtime, &destination).unwrap();
        assert_eq!(fs::read_to_string(destination).unwrap(), before);
        assert!(before.contains("line-0\n"));
        assert!(before.contains("line-159\n"));
        assert_eq!(runtime.read_text().unwrap(), before);
        assert!(save_text(&runtime, &runtime.root.join(LOG_FILE)).is_err());
        assert!(save_text(&runtime, &directory.join("export.pdf")).is_err());
        assert!(save_text(&runtime, &directory.join("export.zip")).is_err());
        runtime.shutdown();
        assert!(directory.starts_with(std::env::temp_dir()));
        fs::remove_dir_all(directory).unwrap();
    }
}
