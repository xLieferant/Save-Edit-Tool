use super::export;
use super::models::ModConflictAnalysisReport;
use super::service;
use crate::shared::user_log;
use crate::state::{AppProfileState, DecryptCache};
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use tauri::{AppHandle, State};

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "unknown panic payload".to_string()
}

#[tauri::command]
pub fn analyze_mod_conflict_diagnostics(
    profile_state: State<'_, AppProfileState>,
    decrypt_cache: State<'_, DecryptCache>,
) -> Result<ModConflictAnalysisReport, String> {
    crate::dev_log!("[diagnostics] command invoked");

    match catch_unwind(AssertUnwindSafe(|| {
        service::analyze_mod_conflict_diagnostics(profile_state.inner(), decrypt_cache.inner())
    })) {
        Ok(Ok(report)) => {
            crate::dev_log!(
                "[diagnostics] command completed successfully: status={} suspects={} missing_refs={} limitations={}",
                report.overview.status_badge,
                report.suspected_mods.len(),
                report.missing_references.len(),
                report.limitations.len()
            );
            Ok(report)
        }
        Ok(Err(error)) => {
            crate::dev_log!("[diagnostics] command returned error: {}", error);
            let _ = user_log::write_user_log(
                &format!("mod_conflict_analyzer failed | {}", error),
                "error",
            );
            Err(error)
        }
        Err(payload) => {
            let message = panic_message(payload);
            let safe_message =
                "Analyzer failed unexpectedly while processing the current data.".to_string();
            crate::dev_log!("[diagnostics] command panic caught: {}", message);
            let _ = user_log::write_user_log(
                &format!("mod_conflict_analyzer panic | {}", message),
                "error",
            );
            Err(safe_message)
        }
    }
}

#[tauri::command]
pub fn export_mod_conflict_diagnostics_report(
    app: AppHandle,
    report: ModConflictAnalysisReport,
    formatted: Option<bool>,
) -> Result<Option<String>, String> {
    let formatted = formatted.unwrap_or(false);
    match catch_unwind(AssertUnwindSafe(|| export::export_report(&app, &report, formatted))) {
        Ok(Ok(path)) => {
            if let Some(path) = path.as_deref() {
                crate::dev_log!(
                    "[diagnostics] export succeeded formatted={} path={}",
                    formatted,
                    path
                );
                let _ = user_log::write_user_log(
                    &format!("mod_conflict_analyzer export success | {}", path),
                    "success",
                );
            } else {
                crate::dev_log!("[diagnostics] export canceled formatted={}", formatted);
            }
            Ok(path)
        }
        Ok(Err(error)) => {
            crate::dev_log!("[diagnostics] export failed formatted={} error={}", formatted, error);
            let _ = user_log::write_user_log(
                &format!("mod_conflict_analyzer export failed | {}", error),
                "error",
            );
            Err(error)
        }
        Err(payload) => {
            let message = panic_message(payload);
            crate::dev_log!("[diagnostics] export panic caught: {}", message);
            let _ = user_log::write_user_log(
                &format!("mod_conflict_analyzer export panic | {}", message),
                "error",
            );
            Err("Analyzer export failed unexpectedly.".to_string())
        }
    }
}
