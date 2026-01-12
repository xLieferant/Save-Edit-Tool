use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;

pub fn write_user_log(action: &str, stage: &str) -> Result<(), String> {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let formatted = format!(
        "[{}] USER ACTION: {} | Stage: {}\n",
        timestamp,
        action,
        stage.to_uppercase()
    );

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("ets2_tool_user.log")
        .map_err(|e| format!("Could not open user log: {}", e))?;

    file.write_all(formatted.as_bytes())
        .map_err(|e| format!("Could not write user log: {}", e))
}
