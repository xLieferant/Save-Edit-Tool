use crate::shared::user_log;
use tauri::command;

pub mod commands {
    use super::*;

    #[command]
    pub fn log_user_action(action: String, stage: String) -> Result<String, String> {
        user_log::write_user_log(&action, &stage)?;
        Ok(format!("Logged action '{}' with stage '{}'", action, stage))
    }
}
