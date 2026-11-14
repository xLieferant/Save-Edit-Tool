use chrono::Local;
use once_cell::sync::Lazy;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[path = "../features/logging/diagnostic_runtime.rs"]
pub mod diagnostic;

pub const MAX_LOG_SIZE_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_LOG_ARCHIVES: usize = 3;
const DEBUG_LOG_FILE_NAME: &str = "application.log.txt";

static LOG_WRITE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[macro_export]
macro_rules! dev_log {
    ($($arg:tt)*) => {{
        $crate::shared::logs::write_log(format!($($arg)*));
    }};
}

fn default_log_root() -> PathBuf {
    if let Some(runtime) = diagnostic::active() {
        return runtime.root.join("logs");
    }
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("com.xlieferant.saveedittool")
        .join("support")
        .join("logs")
}

pub fn log_directory_path() -> Option<PathBuf> {
    Some(default_log_root())
}

pub fn technical_log_path() -> PathBuf {
    default_log_root().join(DEBUG_LOG_FILE_NAME)
}

pub fn ensure_log_directory() -> Result<PathBuf, String> {
    let path = default_log_root();
    fs::create_dir_all(&path).map_err(|error| {
        format!(
            "Could not create log directory {}: {}",
            path.display(),
            error
        )
    })?;
    Ok(path)
}

pub fn ensure_log_file(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create log directory {}: {}",
                parent.display(),
                error
            )
        })?;
    }

    if !path.exists() {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| format!("Could not create log file {}: {}", path.display(), error))?;
    }

    Ok(())
}

fn archived_log_path(path: &Path, index: usize) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| format!("{}.{}.txt", value.trim_end_matches(".txt"), index))
        .unwrap_or_else(|| format!("log.{}", index));

    path.with_file_name(file_name)
}

pub fn clear_log_archives(path: &Path) -> Result<(), String> {
    for index in 1..=MAX_LOG_ARCHIVES {
        let archive = archived_log_path(path, index);
        if archive.exists() {
            fs::remove_file(&archive).map_err(|error| {
                format!(
                    "Could not remove archived log file {}: {}",
                    archive.display(),
                    error
                )
            })?;
        }
    }
    Ok(())
}

pub fn rotate_log_file(path: &Path) -> Result<(), String> {
    ensure_log_file(path)?;

    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "Could not read log file metadata {}: {}",
            path.display(),
            error
        )
    })?;
    if metadata.len() < MAX_LOG_SIZE_BYTES {
        return Ok(());
    }

    for index in (1..=MAX_LOG_ARCHIVES).rev() {
        let archive = archived_log_path(path, index);
        if index == MAX_LOG_ARCHIVES && archive.exists() {
            fs::remove_file(&archive).map_err(|error| {
                format!(
                    "Could not rotate log archive {}: {}",
                    archive.display(),
                    error
                )
            })?;
        }

        let previous = if index == 1 {
            path.to_path_buf()
        } else {
            archived_log_path(path, index - 1)
        };

        if previous.exists() {
            fs::rename(&previous, &archive).map_err(|error| {
                format!(
                    "Could not rotate log file {} to {}: {}",
                    previous.display(),
                    archive.display(),
                    error
                )
            })?;
        }
    }

    Ok(())
}

pub fn append_log_line(path: &Path, entry: &str) -> Result<(), String> {
    let _guard = LOG_WRITE_LOCK
        .lock()
        .map_err(|_| "Log write lock poisoned".to_string())?;
    rotate_log_file(path)?;
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|error| format!("Could not open log file {}: {}", path.display(), error))?;
    file.write_all(entry.as_bytes())
        .map_err(|error| format!("Could not write log file {}: {}", path.display(), error))
}

pub fn write_log(msg: String) {
    if let Some(runtime) = diagnostic::active() {
        runtime.legacy("DEBUG", "backend", &msg);
        return;
    }
    // Before logger initialization, preserve real errors without flooding startup with trace data.
    if !msg.starts_with("[trace] ERROR") {
        return;
    }
    let entry = format!(
        "[{}] [ERROR] [backend] {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        diagnostic::sanitize(&msg)
    );
    let _ = append_log_line(&technical_log_path(), &entry);
}
