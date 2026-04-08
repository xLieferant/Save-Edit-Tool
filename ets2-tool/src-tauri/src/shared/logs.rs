use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

// Wichtig: Dieses Makro exportiert das globale log!-Makro
#[macro_export]
macro_rules! dev_log {
    ($($arg:tt)*) => {{
        $crate::shared::logs::write_log(format!($($arg)*));
    }};
}

fn default_log_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("SimNexusHub")
        .join("ets2_tool.log")
}

pub fn write_log(msg: String) {
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{}] {}\n", timestamp, msg);

    let path = default_log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(&path) {
        let _ = file.write_all(entry.as_bytes());
    }

    println!("{}", entry);
}
