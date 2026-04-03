use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

fn default_user_log_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("SimNexusHub")
        .join("ets2_tool_user.log")
}

pub fn write_user_log(action: &str, stage: &str) -> Result<(), String> {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let formatted = format!(
        "[{}] USER ACTION: {} | Stage: {}\n",
        timestamp,
        action,
        stage.to_uppercase()
    );

    let path = default_user_log_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not open user log: {}", e))?;
    }

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|e| format!("Could not open user log: {}", e))?;

    file.write_all(formatted.as_bytes())
        .map_err(|e| format!("Could not write user log: {}", e))
}
