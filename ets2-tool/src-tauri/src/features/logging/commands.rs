use tauri::{AppHandle, State, command};

use crate::shared::user_log;
use crate::state::AppProfileState;

use super::service;

#[command]
pub fn log_user_action(action: String, stage: String) -> Result<String, String> {
    if let Err(error) = user_log::write_user_log(&action, &stage) {
        crate::dev_log!(
            "[logging] Failed to write user log action='{}' stage='{}': {}",
            action,
            stage,
            error
        );
    }
    Ok(format!("Logged action '{}' with stage '{}'", action, stage))
}

#[command]
pub fn log_diagnostics_event(
    event: String,
    detail: Option<String>,
    user_visible: Option<bool>,
) -> Result<String, String> {
    let normalized_event = event.trim();
    let normalized_detail = detail.unwrap_or_default().trim().to_string();

    if normalized_detail.is_empty() {
        crate::dev_log!("[diagnostics:frontend] {}", normalized_event);
    } else {
        crate::dev_log!(
            "[diagnostics:frontend] {} | {}",
            normalized_event,
            normalized_detail
        );
    }

    if user_visible.unwrap_or(false) {
        let action = if normalized_detail.is_empty() {
            format!("diagnostics: {}", normalized_event)
        } else {
            format!("diagnostics: {} | {}", normalized_event, normalized_detail)
        };
        if let Err(error) = user_log::write_user_log(&action, "error") {
            crate::dev_log!(
                "[logging] Failed to write diagnostics user log event='{}': {}",
                action,
                error
            );
        }
    }

    Ok("ok".to_string())
}

#[command]
pub fn build_support_report(profile_state: State<'_, AppProfileState>) -> Result<String, String> {
    service::build_support_report(profile_state.inner())
}

#[command]
pub fn export_logs_bundle(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
) -> Result<Option<String>, String> {
    service::export_logs_bundle(&app, profile_state.inner())
}
