use tauri::{AppHandle, State, command};

use crate::shared::user_log::{self, UserLogEntry, UserLogStatus};
use crate::state::AppProfileState;

use super::service;

#[command]
pub fn log_user_action(
    action: String,
    stage: String,
    profile_state: State<'_, AppProfileState>,
) -> Result<String, String> {
    let context = service::resolve_active_context(profile_state.inner());
    let normalized_action = action.trim();
    let normalized_stage = stage.trim().to_ascii_lowercase();
    let message = if normalized_stage.is_empty() {
        format!("Frontend action recorded: {}", normalized_action)
    } else {
        format!(
            "Frontend action recorded: {} ({})",
            normalized_action, normalized_stage
        )
    };

    let result = match normalized_stage.as_str() {
        "warning" | "warn" => service::record_warning(
            "frontend_action",
            Some(normalized_action),
            &message,
            None,
            &context,
        ),
        "error" | "failed" | "fail" => service::record_error(
            "frontend_action",
            Some(normalized_action),
            &message,
            None,
            &context,
        ),
        _ => service::record_info("frontend_action", &message, &context),
    };

    if let Err(error) = result {
        crate::dev_log!(
            "[logging] Failed to write frontend action '{}' stage='{}': {}",
            action,
            stage,
            error
        );
        user_log::write_user_log(&action, &stage)?;
    }

    Ok(format!("Logged action '{}' with stage '{}'", action, stage))
}

#[command]
pub fn log_diagnostics_event(
    event: String,
    detail: Option<String>,
    user_visible: Option<bool>,
    profile_state: State<'_, AppProfileState>,
) -> Result<String, String> {
    let normalized_event = event.trim();
    let normalized_detail = detail.unwrap_or_default().trim().to_string();
    let context = service::resolve_active_context(profile_state.inner());

    if normalized_detail.is_empty() {
        crate::dev_log!("[diagnostics:frontend] {}", normalized_event);
    } else {
        crate::dev_log!(
            "[diagnostics:frontend] {} | {}",
            normalized_event,
            normalized_detail
        );
    }

    let message = if normalized_detail.is_empty() {
        format!("Diagnostics event: {}", normalized_event)
    } else {
        format!(
            "Diagnostics event: {} | {}",
            normalized_event, normalized_detail
        )
    };

    if user_visible.unwrap_or(false) {
        let _ = service::record_warning(
            "diagnostics_frontend",
            Some(normalized_event),
            &message,
            None,
            &context,
        );
    } else {
        let _ = user_log::user_log_debug("Diagnostics", message);
    }

    Ok("ok".to_string())
}

#[command]
pub fn get_user_logs(
    level_filter: Option<String>,
    max_lines: Option<u32>,
) -> Result<Vec<UserLogEntry>, String> {
    service::get_user_logs(level_filter, max_lines)
}

#[command]
pub fn export_user_logs(
    app: AppHandle,
    profile_state: State<'_, AppProfileState>,
) -> Result<String, String> {
    service::export_user_logs(&app, profile_state.inner())
}

#[command]
pub fn clear_user_logs() -> Result<Option<String>, String> {
    service::clear_user_logs()
}

#[command]
pub fn get_log_status() -> Result<UserLogStatus, String> {
    service::get_log_status()
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
