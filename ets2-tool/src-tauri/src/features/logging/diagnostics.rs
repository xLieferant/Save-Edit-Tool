use crate::shared::logs::diagnostic::{DiagnosticEvent, storage};
use crate::state::DiagnosticsState;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn diagnostics_record(
    mut event: DiagnosticEvent,
    state: State<'_, DiagnosticsState>,
) -> Result<Option<String>, String> {
    event.original_modified = None;
    event.rollback_verified = None;
    event.backup_id = None;
    event.error_id = None;
    event.session_id.clear();
    event.timestamp.clear();
    Ok(state.0.record(event))
}
#[tauri::command]
pub async fn get_log_preview(state: State<'_, DiagnosticsState>) -> Result<String, String> {
    let runtime = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || runtime.preview(100))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn diagnostics_user_logs(
    known_revision: Option<String>,
    state: State<'_, DiagnosticsState>,
) -> Result<Option<super::report::UserLogSnapshot>, String> {
    let runtime = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        super::report::user_log_snapshot(&runtime, known_revision.as_deref())
    })
    .await
    .map_err(|_| "user_log_report.errors.read_failed".to_string())?
    .map_err(|_| "user_log_report.errors.read_failed".to_string())
}

#[tauri::command]
pub async fn export_log_file(
    app: AppHandle,
    state: State<'_, DiagnosticsState>,
) -> Result<Option<String>, String> {
    let runtime = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || super::report::export_user_report(&runtime, &app))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn diagnostics_cleanup_preview(
    scope: String,
    state: State<'_, DiagnosticsState>,
) -> Result<storage::CleanupPreview, String> {
    let runtime = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || storage::preview(&runtime, &scope))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn diagnostics_cleanup(
    scope: String,
    token: String,
    confirmed: bool,
    state: State<'_, DiagnosticsState>,
) -> Result<u64, String> {
    let runtime = state.0.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        storage::cleanup(&runtime, &scope, &token, confirmed)
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Err(error) = &result {
        let mut event = DiagnosticEvent::new(
            "WARN",
            "cache",
            "clear",
            "Temporary cache could not be cleared.",
        );
        event.technical = Some(error.clone());
        state.0.record(event);
    }
    result
}
